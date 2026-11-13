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
/// The node's data directory on `network`, when the GUI names one: the
/// configured root with the network's subdirectory from core's row, the
/// layout the daemon's own default keeps. An explicit directory is taken as
/// given (IG-Q17), so one shared by two networks would be refused by the
/// network marker at a switch; this keeps each network's store its own.
fn data_dir_for(root: &Path, network: NetworkType) -> PathBuf {
    match network.network().subdirectory() {
        Some(sub) => root.join(sub),
        None => root.to_path_buf(),
    }
}

/// The node's command line: the GUI's network, no admin listener, one view
/// listener on loopback at `rpc_port`, the data directory when configured,
/// and the bundled Tor.
pub(crate) fn sidecar_args(
    config: &DaemonConfig,
    network: NetworkType,
    rpc_port: u16,
    tor_file: Option<&Path>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--network".into(),
        network.as_str().into(),
        "--rpc-admin-listen".into(),
        "none".into(),
        "--rpc-view-listen".into(),
        format!("127.0.0.1:{rpc_port}").into(),
    ];
    if let Some(dir) = &config.data_dir {
        args.push("--data-dir".into());
        args.push(data_dir_for(Path::new(dir), network).into());
    }
    if let Some(tor) = tor_file {
        args.push("--tor-file".into());
        args.push(tor.into());
    }
    args
}

/// Whether the GUI may move to `to` while `open` is the open wallet's
/// network. A wallet is bound to its network, and its node runs that
/// network: a switch to another waits until the wallet is closed, so a
/// wallet of one network never gets a node of another.
pub(crate) fn may_switch(
    open: Option<shekyl_engine_core::Network>,
    to: NetworkType,
) -> Result<(), String> {
    match open {
        Some(open) if open != to.network() => Err(format!(
            "The open wallet is on {}. Close it before switching to {}: a wallet belongs to one network.",
            open.name(),
            to.as_str()
        )),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub keep_running_on_exit: bool,
    #[serde(default)]
    pub data_dir: Option<String>,
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
    /// The network the node was started on (or is being started on).
    network: RwLock<NetworkType>,
    child_pid: RwLock<Option<u32>>,
    mode: RwLock<DaemonMode>,
    config: RwLock<DaemonConfig>,
    config_dir: PathBuf,
}

impl DaemonManager {
    pub fn new(config_dir: PathBuf) -> Arc<Self> {
        let config = DaemonConfig::load(&config_dir);
        Arc::new(Self {
            network: RwLock::new(NetworkType::default()),
            child_pid: RwLock::new(None),
            mode: RwLock::new(DaemonMode::Unavailable),
            config: RwLock::new(config),
            config_dir,
        })
    }

    /// Start the node on `network`: an answering daemon at the network's
    /// port is used as found, and otherwise the bundled one is started.
    pub async fn start(&self, app: &AppHandle, network: NetworkType) {
        *self.network.write().await = network;
        let config = self.config.read().await.clone();
        let rpc_port = network.rpc_port();
        let rpc_url = format!("http://127.0.0.1:{rpc_port}");

        if Self::check_health(&rpc_url).await {
            tracing::info!(port = rpc_port, "external daemon detected");
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
        let args = sidecar_args(&config, network, rpc_port, tor_file.as_deref());

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

    /// Run the node on `network`: nothing to do when it already does;
    /// otherwise a node this GUI started is stopped and the node is started
    /// again on `network`, its own port and its own store. `mine-address`
    /// rides the same relaunch once GS-2 carries it (IG-12: neither is
    /// changeable at runtime).
    pub async fn relaunch(&self, app: &AppHandle, network: NetworkType) {
        if *self.network.read().await == network {
            return;
        }
        if let Some(pid) = self.child_pid.write().await.take() {
            tracing::info!(
                pid,
                to = network.as_str(),
                "relaunching shekyld on another network"
            );
            // `kill_process` waits out the node's stop; not on the runtime.
            let _ = tokio::task::spawn_blocking(move || Self::kill_process(pid)).await;
        }
        *self.mode.write().await = DaemonMode::Unavailable;
        self.start(app, network).await;
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
        let rpc_url = format!("http://127.0.0.1:{}", self.network.read().await.rpc_port());

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

    /// Whether a daemon answers at `rpc_url`: its `version` read, which
    /// every connection is served (shekyl-core `RPC_CHANNEL.md` §6.1).
    async fn check_health(rpc_url: &str) -> bool {
        let probe = crate::daemon_rpc::call::<shekyl_rpc_client::wire::Version>(
            rpc_url,
            shekyl_rpc_types::kinds::NoParams,
            "the node's version",
        );
        matches!(
            tokio::time::timeout(Duration::from_secs(2), probe).await,
            Ok(Ok(_))
        )
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
    use shekyl_engine_core::Network;

    fn config(data_dir: Option<&str>) -> DaemonConfig {
        DaemonConfig {
            keep_running_on_exit: false,
            data_dir: data_dir.map(str::to_owned),
        }
    }

    #[test]
    fn the_sidecar_names_the_bundled_tor() {
        let tor = bundled_tor(Path::new("/res"));
        let args = sidecar_args(&config(None), NetworkType::Testnet, 12029, Some(&tor));
        let want: Vec<OsString> = [
            "--network",
            "testnet",
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

    /// Every network's launch names that network and listens on its port
    /// from core's row. Red edit: drop `--network` (the node then runs
    /// mainnet whatever the wallet's network).
    #[test]
    fn the_launch_carries_the_wallet_s_network_and_its_port() {
        for net in NetworkType::ALL {
            let args = sidecar_args(&config(None), net, net.rpc_port(), None);
            assert_eq!(
                args[..2],
                [OsString::from("--network"), OsString::from(net.as_str())]
            );
            assert_eq!(
                args[5],
                OsString::from(format!("127.0.0.1:{}", net.network().ports().rpc))
            );
        }
        assert_eq!(
            NetworkType::default(),
            NetworkType::Mainnet,
            "Rick, 2026-11-13"
        );
    }

    /// A configured data directory is per network: the root for mainnet,
    /// the network's subdirectory from core's row otherwise, so a switch
    /// never points two networks at one store.
    #[test]
    fn a_configured_data_dir_is_per_network() {
        let root = Path::new("/data");
        let dirs: Vec<PathBuf> = NetworkType::ALL
            .iter()
            .map(|n| data_dir_for(root, *n))
            .collect();
        for (n, dir) in NetworkType::ALL.iter().zip(&dirs) {
            let want = n
                .network()
                .subdirectory()
                .map_or_else(|| root.to_path_buf(), |sub| root.join(sub));
            assert_eq!(*dir, want);
        }
        let distinct: std::collections::BTreeSet<_> = dirs.iter().collect();
        assert_eq!(
            distinct.len(),
            dirs.len(),
            "one store per network: {dirs:?}"
        );
        let args = sidecar_args(&config(Some("/data")), NetworkType::Testnet, 12029, None);
        let at = args.iter().position(|a| a == "--data-dir").unwrap();
        assert_eq!(
            PathBuf::from(&args[at + 1]),
            data_dir_for(root, NetworkType::Testnet)
        );
    }

    /// The node follows a switch only while no wallet of another network is
    /// open: a wallet of one network never gets a node of another.
    #[test]
    fn a_wallet_of_one_network_never_gets_a_node_of_another() {
        assert!(may_switch(None, NetworkType::Testnet).is_ok());
        assert!(may_switch(Some(Network::Testnet), NetworkType::Testnet).is_ok());
        let refused = may_switch(Some(Network::Testnet), NetworkType::Mainnet).unwrap_err();
        assert!(
            refused.contains("Close it before switching to mainnet"),
            "{refused}"
        );
    }

    #[test]
    fn without_a_resource_directory_no_tor_file_is_named() {
        let args = sidecar_args(&config(None), NetworkType::Mainnet, 11029, None);
        assert!(!args.iter().any(|a| a == "--tor-file"));
    }
}
