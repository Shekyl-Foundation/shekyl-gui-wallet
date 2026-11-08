// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;
use tokio::sync::RwLock;

use crate::state::NetworkType;

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEALTH_POLL_TIMEOUT: Duration = Duration::from_secs(30);

/// Tor's file name in the installer's `tor/` resource directory: core's
/// gitian archive for the target, staged by `scripts/release/core_archive.py`.
const TOR_FILE: &str = if cfg!(windows) { "tor.exe" } else { "tor" };

/// The bundled Tor: `tor/` under the app's resource directory, wherever the
/// bundle type puts that (`Contents/Resources` in the `.app`, beside the
/// executable on Windows, `/usr/lib/<product>` in the `.deb`).
fn bundled_tor(resource_dir: &Path) -> PathBuf {
    resource_dir.join("tor").join(TOR_FILE)
}

/// The sidecar's command line.
///
/// The daemon's grammar (shekyl-core `docs/design/INPUT_GRAMMAR.md` §5): one
/// view listener on loopback for the wallet and no admin listener, so
/// nothing on this machine can mine or ban through the sidecar. Nothing
/// reads the daemon's stdin without `--console`.
///
/// `--tor-file` names the bundled Tor explicitly, one mechanism on every
/// platform (GUI_SYNC_LANE.md §4, GS-1). The daemon checks that file
/// against its pin at every spawn; a file it cannot pin is a warning and
/// the node holds its own transactions until Tor is up (core TB-2), never
/// a refusal to start.
fn sidecar_args(config: &DaemonConfig, tor_file: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--rpc-admin-listen".into(),
        "none".into(),
        "--rpc-view-listen".into(),
        format!("127.0.0.1:{}", config.rpc_port).into(),
    ];
    if let Some(dir) = &config.data_dir {
        args.push("--data-dir".into());
        args.push(dir.into());
    }
    if let Some(tor) = tor_file {
        args.push("--tor-file".into());
        args.push(tor.into());
    }
    args
}

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

        let tor_file = match app.path().resource_dir() {
            Ok(dir) => Some(bundled_tor(&dir)),
            Err(e) => {
                // Without the resource directory the GUI cannot name its
                // Tor; the daemon then searches for one and announces what
                // it found.
                tracing::warn!("no resource directory for the bundled Tor: {e}");
                None
            }
        };
        let args = sidecar_args(&config, tor_file.as_deref());

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

#[cfg(test)]
mod tests {
    use super::*;

    fn config(data_dir: Option<&str>) -> DaemonConfig {
        DaemonConfig {
            keep_running_on_exit: false,
            data_dir: data_dir.map(str::to_owned),
            rpc_port: 12029,
        }
    }

    #[test]
    fn the_sidecar_names_the_bundled_tor() {
        let tor = bundled_tor(Path::new("/res"));
        let args = sidecar_args(&config(None), Some(&tor));
        let want: Vec<OsString> = [
            "--rpc-admin-listen",
            "none",
            "--rpc-view-listen",
            "127.0.0.1:12029",
            "--tor-file",
        ]
        .iter()
        .map(OsString::from)
        .chain([tor.clone().into_os_string()])
        .collect();
        assert_eq!(args, want);
        assert_eq!(tor, Path::new("/res").join("tor").join(TOR_FILE));
    }

    #[test]
    fn the_data_dir_comes_before_the_tor_file() {
        let tor = PathBuf::from("/res/tor/tor");
        let args = sidecar_args(&config(Some("/data")), Some(&tor));
        assert_eq!(
            args[4..],
            ["--data-dir", "/data", "--tor-file", "/res/tor/tor"].map(OsString::from)
        );
    }

    #[test]
    fn without_a_resource_directory_no_tor_file_is_named() {
        let args = sidecar_args(&config(None), None);
        assert!(!args.iter().any(|a| a == "--tor-file"));
    }
}
