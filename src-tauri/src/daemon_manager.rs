// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use tokio::sync::RwLock;

use crate::state::NetworkType;

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub keep_running_on_exit: bool,
    #[serde(default)]
    pub data_dir: Option<String>,
    #[serde(default = "default_rpc_port")]
    pub rpc_port: u16,
}

fn default_rpc_port() -> u16 {
    NetworkType::default().default_rpc_port()
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            keep_running_on_exit: false,
            data_dir: None,
            rpc_port: default_rpc_port(),
        }
    }
}

impl DaemonConfig {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("daemon.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), String> {
        let path = config_dir.join("daemon.json");
        std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonMode {
    Managed,
    External,
    Unavailable,
}

#[derive(Debug, Serialize, Clone)]
pub struct DaemonStatus {
    pub mode: DaemonMode,
    pub ready: bool,
    pub height: u64,
}

pub struct DaemonManager {
    child_pid: RwLock<Option<u32>>,
    mode: RwLock<DaemonMode>,
    config: RwLock<DaemonConfig>,
    config_dir: PathBuf,
}

impl DaemonManager {
    pub fn new(config_dir: PathBuf) -> Arc<Self> {
        let config = DaemonConfig::load(&config_dir);
        Arc::new(Self {
            child_pid: RwLock::new(None),
            mode: RwLock::new(DaemonMode::Unavailable),
            config: RwLock::new(config),
            config_dir,
        })
    }

    pub async fn start(&self, app: &AppHandle) {
        let config = self.config.read().await.clone();
        let rpc_url = format!("http://127.0.0.1:{}/get_info", config.rpc_port);

        if Self::check_health(&rpc_url).await {
            tracing::info!(port = config.rpc_port, "external daemon detected");
            *self.mode.write().await = DaemonMode::External;
            return;
        }

        let sidecar = match app.shell().sidecar("shekyld") {
            Ok(cmd) => cmd,
            Err(e) => {
                tracing::warn!("shekyld sidecar not available: {e}. Run daemon manually or set path in Settings.");
                *self.mode.write().await = DaemonMode::Unavailable;
                return;
            }
        };

        let mut args = vec![
            "--rpc-bind-ip=127.0.0.1".to_string(),
            format!("--rpc-bind-port={}", config.rpc_port),
            "--non-interactive".to_string(),
            "--restricted-rpc".to_string(),
        ];
        if let Some(ref dir) = config.data_dir {
            args.push(format!("--data-dir={dir}"));
        }

        match sidecar.args(&args).spawn() {
            Ok((mut rx, child)) => {
                let pid = child.pid();
                tracing::info!(pid, "spawned shekyld sidecar");
                *self.child_pid.write().await = Some(pid);
                *self.mode.write().await = DaemonMode::Managed;

                tokio::spawn(async move {
                    while let Some(event) = rx.recv().await {
                        match event {
                            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                                tracing::debug!(target: "shekyld", "{}", String::from_utf8_lossy(&line));
                            }
                            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                                tracing::warn!(target: "shekyld", "{}", String::from_utf8_lossy(&line));
                            }
                            tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                                tracing::info!(target: "shekyld", code = ?payload.code, "daemon exited");
                                break;
                            }
                            _ => {}
                        }
                    }
                });

                let elapsed = tokio::time::Instant::now();
                while elapsed.elapsed() < HEALTH_POLL_TIMEOUT {
                    if Self::check_health(&rpc_url).await {
                        tracing::info!("shekyld ready after {:?}", elapsed.elapsed());
                        return;
                    }
                    tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
                }
                tracing::warn!("shekyld health-check timed out after {HEALTH_POLL_TIMEOUT:?}");
            }
            Err(e) => {
                tracing::error!("failed to spawn shekyld: {e}");
                *self.mode.write().await = DaemonMode::Unavailable;
            }
        }
    }

    pub async fn shutdown(&self) {
        let mode = *self.mode.read().await;
        if mode != DaemonMode::Managed {
            return;
        }

        let config = self.config.read().await;
        if config.keep_running_on_exit {
            tracing::info!("keep_running_on_exit is set, leaving shekyld running");
            return;
        }
        drop(config);

        if let Some(pid) = self.child_pid.write().await.take() {
            tracing::info!(pid, "shutting down managed shekyld");
            Self::kill_process(pid);
            *self.mode.write().await = DaemonMode::Unavailable;
        }
    }

    pub async fn status(&self) -> DaemonStatus {
        let config = self.config.read().await;
        let rpc_url = format!("http://127.0.0.1:{}/get_info", config.rpc_port);
        drop(config);

        let ready = Self::check_health(&rpc_url).await;
        DaemonStatus {
            mode: *self.mode.read().await,
            ready,
            height: 0,
        }
    }

    pub async fn config(&self) -> DaemonConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: DaemonConfig) -> Result<(), String> {
        new_config.save(&self.config_dir)?;
        *self.config.write().await = new_config;
        Ok(())
    }

    async fn check_health(rpc_url: &str) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build();
        let Ok(client) = client else { return false };
        client.get(rpc_url).send().await.is_ok()
    }

    #[cfg(unix)]
    fn kill_process(pid: u32) {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        std::thread::sleep(Duration::from_secs(2));
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    fn kill_process(pid: u32) {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
}
