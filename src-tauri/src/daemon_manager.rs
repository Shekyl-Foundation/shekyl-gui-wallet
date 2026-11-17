// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use shekyl_rpc_client::wire::DaemonStop;
use shekyl_rpc_transport::local::{LocalFault, LocalTarget};
use shekyl_rpc_types::kinds::NoParams;
use tokio::sync::RwLock;

use crate::node_launch::{self, Launch, Life, OwnNode};
use crate::state::{NetworkType, NodeGate};

/// Spacing of the looks at a found node's rendezvous while it stops, and of
/// the starts retried while it still holds the data directory. This wallet
/// did not start that node and holds no connection whose end would say it
/// is gone, so it looks.
const FOUND_NODE_POLL: Duration = Duration::from_millis(500);

/// The diagnostic a node refuses its start with while another process
/// holds its data directory (shekyl-core `shekyl-options`, `DataDirLocked`).
pub(crate) const DATA_DIR_LOCKED: &str = "config.data-dir.locked";

/// What is said of a node that went without a word of its own.
const NODE_STOPPED: &str = "The wallet's node stopped.";

/// What the engine's way to the node says before the node has started.
const NODE_NOT_STARTED: &str = "The wallet's node has not started.";

const TOR_FILE: &str = if cfg!(windows) { "tor.exe" } else { "tor" };
const NODE_FILE: &str = if cfg!(windows) {
    "shekyld.exe"
} else {
    "shekyld"
};

fn bundled_tor(resource_dir: &Path) -> PathBuf {
    resource_dir.join("tor").join(TOR_FILE)
}

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

/// What a launch names besides the network and the configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaunchNames {
    /// The bundled Tor (`--tor-file`).
    pub tor_file: Option<PathBuf>,
    /// The address the node's miner pays (`--mine-address`). The daemon
    /// reads it only at its start (shekyl-core `INPUT_GRAMMAR.md` IG-12),
    /// so a change is a relaunch.
    pub mine_address: Option<String>,
    /// The directory for the owner's socket (`--rpc-socket-dir`, Unix): a
    /// test's own, so its node never meets a node of the account it runs
    /// under. The wallet names none.
    #[cfg(test)]
    pub socket_dir: Option<PathBuf>,
}

/// The node's command line, less the owning connection the launcher adds.
///
/// The daemon's grammar (shekyl-core `docs/design/INPUT_GRAMMAR.md` §5): the
/// GUI's network; no admin listener; one view listener on loopback at
/// `rpc_port`, which the wallet engine reads until its calls move to the
/// owner's leg (core `RPC_CHANNEL.md` RT-W10 step 5); the data directory when
/// configured. The wallet's own calls go over the owner's leg, which the
/// daemon serves without being asked.
///
/// `--tor-file` names the bundled Tor explicitly, one mechanism on every
/// platform (GUI_SYNC_LANE.md §4, GS-1). The daemon checks that file
/// against its pin at every spawn; a file it cannot pin is a warning and
/// the node holds its own transactions until Tor is up (core TB-2), never
/// a refusal to start.
pub(crate) fn sidecar_args(
    config: &DaemonConfig,
    network: NetworkType,
    rpc_port: u16,
    names: &LaunchNames,
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
    if let Some(tor) = &names.tor_file {
        args.push("--tor-file".into());
        args.push(tor.into());
    }
    if let Some(address) = &names.mine_address {
        args.push("--mine-address".into());
        args.push(address.into());
    }
    #[cfg(test)]
    if let Some(dir) = &names.socket_dir {
        args.push("--rpc-socket-dir".into());
        args.push(dir.into());
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

/// What a node that outlives the wallet is asked for before it exists
/// (GUI_SYNC_LANE.md `GS-Q2`).
pub(crate) const KEEP_RUNNING_NOT_YET: &str =
    "Keeping the node running after the wallet closes is not yet implemented. \
     The node this wallet starts stops when the wallet closes.";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonConfig {
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

/// Where the wallet's own node stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodeState {
    /// Nothing has been started yet.
    Idle,
    /// The wallet's node is started and has not said it is ready.
    Starting,
    /// The wallet's node is ready: its owner's leg is served.
    Running,
    /// A node is stopping, and the wallet's own starts when it is gone.
    Stopping,
    /// A node of this account already runs on the network. The wallet did
    /// not start it and does not use it.
    Found,
    /// Something that is not this account's node answers at the node's
    /// local address. Nothing is started, and nothing is retried.
    Refused(String),
    /// The node did not start, or stopped by itself. The words are its own.
    Failed(String),
    /// This build has no node to start.
    Unavailable(String),
}

impl NodeState {
    const fn keyword(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Found => "found",
            Self::Refused(_) => "refused",
            Self::Failed(_) => "failed",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// The node's state for the page.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct DaemonStatus {
    /// `idle`, `starting`, `running`, `stopping`, `found`, `refused`,
    /// `failed` or `unavailable`.
    pub state: &'static str,
    /// The state in words, where there is something to say.
    pub message: Option<String>,
    /// The network the node runs, or is to run.
    pub network: &'static str,
    /// Whether a force stop has a node to act on: one this wallet started.
    pub can_force_stop: bool,
}

/// The words for a node of this account the wallet found running.
fn found_words(network: NetworkType) -> String {
    format!(
        "A Shekyl node of yours is already running on {}. This wallet uses only the node it \
         starts itself.",
        network.as_str()
    )
}

/// What the launcher starts, fixed when the app starts.
#[derive(Clone, Debug, Default)]
pub(crate) struct LaunchEnv {
    /// The bundled node, when this build has one.
    pub program: Option<PathBuf>,
    /// The bundled Tor.
    pub tor_file: Option<PathBuf>,
    /// A test's own socket directory (see [`LaunchNames`]).
    #[cfg(test)]
    pub socket_dir: Option<PathBuf>,
    /// A test's view-listener port, so two tests' nodes never share one.
    #[cfg(test)]
    pub rpc_port: Option<u16>,
    /// A test's further arguments: no traffic.
    #[cfg(test)]
    pub extra_args: Vec<OsString>,
}

impl LaunchEnv {
    /// The node beside this executable, where the bundle puts it, and the
    /// Tor in the resource directory.
    pub(crate) fn bundled(resource_dir: Option<&Path>) -> Self {
        let program = tauri::utils::platform::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(NODE_FILE)))
            // A build with no node has none, or CI's empty placeholder.
            .filter(|node| std::fs::metadata(node).is_ok_and(|m| m.is_file() && m.len() > 0));
        Self {
            program,
            tor_file: resource_dir.map(bundled_tor),
            #[cfg(test)]
            socket_dir: None,
            #[cfg(test)]
            rpc_port: None,
            #[cfg(test)]
            extra_args: Vec::new(),
        }
    }
}

struct Inner {
    /// The network the node runs, or is to run.
    network: NetworkType,
    /// The address the running node's miner pays.
    mine_address: Option<String>,
    state: NodeState,
    /// The node this wallet started, while it lives.
    node: Option<Arc<OwnNode>>,
    /// Counts launches, so a launch's follower never writes over a later
    /// launch's state.
    launch: u64,
}

pub struct DaemonManager {
    env: LaunchEnv,
    /// Held across a whole start, relaunch or replacement, so two of them
    /// never interleave.
    lifecycle: tokio::sync::Mutex<()>,
    inner: RwLock<Inner>,
    config: RwLock<DaemonConfig>,
    config_dir: PathBuf,
    gate: NodeGate,
}

impl DaemonManager {
    pub(crate) fn new(config_dir: PathBuf, env: LaunchEnv, gate: NodeGate) -> Arc<Self> {
        let config = DaemonConfig::load(&config_dir);
        gate.set(Some(NODE_NOT_STARTED.to_owned()));
        Arc::new(Self {
            env,
            lifecycle: tokio::sync::Mutex::new(()),
            inner: RwLock::new(Inner {
                network: NetworkType::default(),
                mine_address: None,
                state: NodeState::Idle,
                node: None,
                launch: 0,
            }),
            config: RwLock::new(config),
            config_dir,
            gate,
        })
    }

    /// "This computer" on `network`: the owner's leg of this account's node.
    fn target(&self, network: NetworkType) -> Result<LocalTarget, LocalFault> {
        #[cfg(test)]
        if let Some(dir) = &self.env.socket_dir {
            return LocalTarget::resolve(
                network.network(),
                &shekyl_rpc_rendezvous::Instance::Default,
                Some(dir),
            );
        }
        LocalTarget::this_computer(network.network())
    }

    /// The owner's leg of the wallet's own node, for a call: only while the
    /// node this wallet started is ready. Otherwise the state, in words.
    pub(crate) async fn own_target(&self) -> Result<LocalTarget, String> {
        let inner = self.inner.read().await;
        match &inner.state {
            NodeState::Running => self.target(inner.network).map_err(|e| e.to_string()),
            NodeState::Idle => Err(NODE_NOT_STARTED.to_owned()),
            NodeState::Starting => Err("The wallet's node is starting.".to_owned()),
            NodeState::Stopping => Err("The node is stopping.".to_owned()),
            NodeState::Found => Err(found_words(inner.network)),
            NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => Err(words.clone()),
        }
    }

    async fn set_state(&self, state: NodeState) {
        let mut inner = self.inner.write().await;
        // The engine's way to the node on this computer is open only while
        // the node there is the one this wallet started.
        self.gate.set(match &state {
            NodeState::Starting | NodeState::Running => None,
            NodeState::Idle => Some(NODE_NOT_STARTED.to_owned()),
            NodeState::Stopping => Some("The node is stopping.".to_owned()),
            NodeState::Found => Some(found_words(inner.network)),
            NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => Some(words.clone()),
        });
        inner.state = state;
    }

    /// Start the node on `network`.
    pub async fn start(self: &Arc<Self>, network: NetworkType) {
        let _lifecycle = self.lifecycle.lock().await;
        self.inner.write().await.network = network;
        self.start_locked().await;
    }

    /// Start the wallet's own node again after it failed or was stopped.
    pub async fn start_again(self: &Arc<Self>) {
        let _lifecycle = self.lifecycle.lock().await;
        if self.inner.read().await.node.is_none() {
            self.start_locked().await;
        }
    }

    /// One target, never a ladder (core `RPC_CHANNEL.md` RT-15): dial "this
    /// computer" first. Nothing there, and the wallet starts its own node.
    /// A node of this account there is found and left alone. Anything else
    /// there is a hard stop.
    async fn start_locked(self: &Arc<Self>) {
        let network = self.inner.read().await.network;
        match self.dial(network).await {
            Ok(()) => {
                tracing::info!(
                    network = network.as_str(),
                    "a node of this account is already running; it is not used"
                );
                self.set_state(NodeState::Found).await;
            }
            Err(
                LocalFault::NoPlace { .. }
                | LocalFault::NotRunning { .. }
                | LocalFault::Stale { .. },
            ) => {
                self.launch_locked().await;
            }
            Err(fault) if fault.is_security_failure() => {
                tracing::error!("the node's local address failed its check: {fault}");
                self.set_state(NodeState::Refused(fault.to_string())).await;
            }
            Err(fault) => {
                self.set_state(NodeState::Failed(fault.to_string())).await;
            }
        }
    }

    /// Dial "this computer" on `network`, and close the connection.
    async fn dial(&self, network: NetworkType) -> Result<(), LocalFault> {
        let target = self.target(network)?;
        tokio::task::spawn_blocking(move || target.dial().map(drop))
            .await
            .unwrap_or_else(|e| Err(LocalFault::Unreachable(e.to_string())))
    }

    /// Launch the wallet's own node and follow its life.
    async fn launch_locked(self: &Arc<Self>) {
        let Some(program) = self.env.program.clone() else {
            self.set_state(NodeState::Unavailable(
                "This build of the wallet carries no node to start.".to_owned(),
            ))
            .await;
            return;
        };
        let config = self.config.read().await.clone();
        let (network, mine_address) = {
            let inner = self.inner.read().await;
            (inner.network, inner.mine_address.clone())
        };
        let names = LaunchNames {
            tor_file: self.env.tor_file.clone(),
            mine_address,
            #[cfg(test)]
            socket_dir: self.env.socket_dir.clone(),
        };
        let rpc_port = network.rpc_port();
        #[cfg(test)]
        let rpc_port = self.env.rpc_port.unwrap_or(rpc_port);
        #[allow(unused_mut, reason = "a test's launch adds its own arguments")]
        let mut args = sidecar_args(&config, network, rpc_port, &names);
        #[cfg(test)]
        args.extend(self.env.extra_args.iter().cloned());
        let launch = Launch {
            program,
            args,
            stderr: self.config_dir.join("shekyld.stderr"),
        };
        if let Err(e) = std::fs::create_dir_all(&self.config_dir) {
            self.set_state(NodeState::Failed(format!(
                "The wallet's settings directory {} could not be made: {e}",
                self.config_dir.display()
            )))
            .await;
            return;
        }
        // The spawn takes the process's spawn lock and does not wait.
        let node = match tokio::task::spawn_blocking(move || node_launch::launch(&launch)).await {
            Ok(Ok(node)) => Arc::new(node),
            Ok(Err(words)) => {
                tracing::error!("{words}");
                self.set_state(NodeState::Failed(words)).await;
                return;
            }
            Err(e) => {
                self.set_state(NodeState::Failed(format!(
                    "The node could not be started: {e}"
                )))
                .await;
                return;
            }
        };
        tracing::info!(
            pid = node.pid(),
            network = network.as_str(),
            "started the wallet's node"
        );
        let life = node.life();
        let id = {
            let mut inner = self.inner.write().await;
            inner.launch += 1;
            inner.node = Some(node);
            inner.launch
        };
        self.set_state(NodeState::Starting).await;
        let manager = Arc::clone(self);
        tokio::spawn(async move { manager.follow(id, life).await });
    }

    /// Follow launch `id`'s life into the state: ready, then gone. A stop
    /// this wallet asked for leaves the state to the flow that asked.
    async fn follow(&self, id: u64, mut life: tokio::sync::watch::Receiver<Life>) {
        loop {
            let now = life.borrow_and_update().clone();
            match now {
                Life::Starting => {}
                Life::Ready => {
                    let mut inner = self.inner.write().await;
                    if inner.launch == id && inner.state == NodeState::Starting {
                        inner.state = NodeState::Running;
                        tracing::info!("the wallet's node is ready");
                    }
                }
                Life::Gone { refusal } => {
                    let mut inner = self.inner.write().await;
                    if inner.launch == id {
                        inner.node = None;
                        if inner.state != NodeState::Stopping {
                            let words = refusal.unwrap_or_else(|| NODE_STOPPED.to_owned());
                            tracing::warn!("the wallet's node is gone: {words}");
                            self.gate.set(Some(words.clone()));
                            inner.state = NodeState::Failed(words);
                        }
                    }
                    return;
                }
            }
            if life.changed().await.is_err() {
                return;
            }
        }
    }

    /// Ask the wallet's own node to stop and wait until it is gone: the
    /// end of stream on its owning connection, which comes when its process
    /// has exited. No timer; a person's force stop is the only kill.
    async fn stop_own_locked(&self) {
        let node = self.inner.read().await.node.clone();
        let Some(node) = node else { return };
        self.set_state(NodeState::Stopping).await;
        tracing::info!(pid = node.pid(), "asking the wallet's node to stop");
        node.ask_to_stop();
        let mut life = node.life();
        // An error is the sender gone, which is the reader's end too.
        drop(life.wait_for(|l| matches!(l, Life::Gone { .. })).await);
        self.inner.write().await.node = None;
    }

    /// Run the node on `network`: nothing to do when it already does;
    /// otherwise a node this wallet started is stopped, and the node is
    /// started again on `network`, its own port and its own store.
    pub async fn relaunch(self: &Arc<Self>, network: NetworkType) {
        let _lifecycle = self.lifecycle.lock().await;
        if self.inner.read().await.network == network {
            return;
        }
        self.stop_own_locked().await;
        self.inner.write().await.network = network;
        self.start_locked().await;
    }

    /// Have the wallet's node pay its miner to `address`. The daemon reads
    /// `mine-address` only at its start, so a node running with another
    /// address is stopped and started again with this one. Returns once the
    /// node is ready, or with the reason it is not.
    pub async fn mine_to(self: &Arc<Self>, address: &str) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        {
            let inner = self.inner.read().await;
            match &inner.state {
                NodeState::Running if inner.mine_address.as_deref() == Some(address) => {
                    return Ok(())
                }
                NodeState::Running => {}
                NodeState::Found => return Err(found_words(inner.network)),
                NodeState::Refused(words) | NodeState::Unavailable(words) => {
                    return Err(words.clone())
                }
                NodeState::Idle
                | NodeState::Starting
                | NodeState::Stopping
                | NodeState::Failed(_) => {}
            }
        }
        self.stop_own_locked().await;
        self.inner.write().await.mine_address = Some(address.to_owned());
        self.start_locked().await;
        self.ready_or_words().await
    }

    /// A wallet with `address` was opened. A node whose miner pays another
    /// address is started again to pay this one; a node that pays none yet
    /// is left as it is.
    pub async fn follow_wallet(self: &Arc<Self>, address: &str) -> Result<(), String> {
        let pays_another = self
            .inner
            .read()
            .await
            .mine_address
            .as_deref()
            .is_some_and(|paid| paid != address);
        if pays_another {
            self.mine_to(address).await
        } else {
            Ok(())
        }
    }

    /// Wait for the node just started to be ready, or say why it is not.
    async fn ready_or_words(&self) -> Result<(), String> {
        let node = self.inner.read().await.node.clone();
        if let Some(node) = node {
            let mut life = node.life();
            drop(life.wait_for(|l| !matches!(l, Life::Starting)).await);
            let now = life.borrow().clone();
            match now {
                Life::Ready => {
                    // The follower writes the same state; this does not
                    // wait for it.
                    let mut inner = self.inner.write().await;
                    if inner.state == NodeState::Starting {
                        inner.state = NodeState::Running;
                    }
                    return Ok(());
                }
                // The node's own words, from the read that ended: the
                // follower may not have written the state yet.
                Life::Gone { refusal } => {
                    return Err(refusal.unwrap_or_else(|| NODE_STOPPED.to_owned()))
                }
                Life::Starting => {}
            }
        }
        let inner = self.inner.read().await;
        Err(match &inner.state {
            NodeState::Found => found_words(inner.network),
            NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => words.clone(),
            _ => "The wallet's node did not start.".to_owned(),
        })
    }

    /// Stop the node of this account the wallet found running, and start
    /// the wallet's own (Rick, 2026-11-17). The stop is `daemon_stop` over
    /// the owner's leg. The wallet holds no connection to that node whose
    /// end would say it is gone, so it looks at the rendezvous until it is,
    /// and starts its own again while that node still holds the data
    /// directory.
    pub async fn replace_found(self: &Arc<Self>) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        let network = {
            let inner = self.inner.read().await;
            if inner.state != NodeState::Found {
                return Err("No other node of yours is running to stop.".to_owned());
            }
            inner.network
        };
        let target = self.target(network).map_err(|e| e.to_string())?;
        let stop = tokio::task::spawn_blocking(move || target.call::<DaemonStop>(&NoParams))
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = stop {
            let words = format!(
                "Your node on {} did not take the stop: {e}",
                network.as_str()
            );
            if let shekyl_rpc_transport::JsonRpcError::Transport(fault) = &e {
                if fault.is_security_failure() {
                    self.set_state(NodeState::Refused(fault.to_string())).await;
                }
            }
            return Err(words);
        }
        self.set_state(NodeState::Stopping).await;
        tracing::info!(network = network.as_str(), "stopping the found node");
        loop {
            match self.dial(network).await {
                Ok(()) | Err(LocalFault::Unreachable(_)) => {
                    tokio::time::sleep(FOUND_NODE_POLL).await;
                }
                Err(fault) if fault.is_security_failure() => {
                    self.set_state(NodeState::Refused(fault.to_string())).await;
                    return Err(fault.to_string());
                }
                Err(_) => break,
            }
        }
        loop {
            self.launch_locked().await;
            match self.ready_or_words().await {
                Ok(()) => return Ok(()),
                // The stopped node is still closing its store.
                Err(words) if words.contains(DATA_DIR_LOCKED) => {
                    self.set_state(NodeState::Stopping).await;
                    tokio::time::sleep(FOUND_NODE_POLL).await;
                }
                Err(words) => return Err(words),
            }
        }
    }

    /// Kill the node this wallet started. A person asks for this; nothing
    /// else does. It takes no lifecycle lock: the flow waiting for the node
    /// to be gone holds it, and sees the node go.
    pub async fn force_stop(&self) -> Result<(), String> {
        let node = self.inner.read().await.node.clone();
        match node {
            Some(node) => {
                tracing::warn!(pid = node.pid(), "force stop: killing the wallet's node");
                node.force_stop()
            }
            None => Err("The wallet has no node of its own to stop.".to_owned()),
        }
    }

    /// The wallet is closing: ask its node to stop, and do not wait. The
    /// node stops cleanly on its own, and this process's exit closes the
    /// connection whatever happens here.
    pub async fn shutdown(&self) {
        if let Some(node) = self.inner.read().await.node.clone() {
            tracing::info!(pid = node.pid(), "the wallet is closing; its node stops");
            node.ask_to_stop();
        }
    }

    pub async fn status(&self) -> DaemonStatus {
        let inner = self.inner.read().await;
        let message = match &inner.state {
            NodeState::Found => Some(found_words(inner.network)),
            NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => Some(words.clone()),
            NodeState::Idle | NodeState::Starting | NodeState::Running | NodeState::Stopping => {
                None
            }
        };
        DaemonStatus {
            state: inner.state.keyword(),
            message,
            network: inner.network.as_str(),
            can_force_stop: inner.node.is_some(),
        }
    }

    /// The process id of the node this wallet started, for a test that
    /// checks the process is gone.
    #[cfg(test)]
    pub(crate) async fn node_pid(&self) -> Option<u32> {
        self.inner.read().await.node.as_ref().map(|n| n.pid())
    }

    pub async fn config(&self) -> DaemonConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: DaemonConfig) -> Result<(), String> {
        new_config.save(&self.config_dir)?;
        *self.config.write().await = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_core::Network;

    fn config(data_dir: Option<&str>) -> DaemonConfig {
        DaemonConfig {
            data_dir: data_dir.map(str::to_owned),
        }
    }

    fn names(tor_file: Option<PathBuf>) -> LaunchNames {
        LaunchNames {
            tor_file,
            ..LaunchNames::default()
        }
    }

    #[test]
    fn the_sidecar_names_the_bundled_tor() {
        let tor = bundled_tor(Path::new("/res"));
        let args = sidecar_args(
            &config(None),
            NetworkType::Testnet,
            12029,
            &names(Some(tor.clone())),
        );
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
            let args = sidecar_args(&config(None), net, net.rpc_port(), &names(None));
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
        let args = sidecar_args(
            &config(Some("/data")),
            NetworkType::Testnet,
            12029,
            &names(None),
        );
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
        let args = sidecar_args(&config(None), NetworkType::Mainnet, 11029, &names(None));
        assert!(!args.iter().any(|a| a == "--tor-file"));
    }

    /// The miner's address rides the launch line: the daemon reads it only
    /// at its start. And the wallet's launch never opens an admin listener:
    /// its own calls go over the owner's leg.
    #[test]
    fn the_launch_names_the_mining_address_and_no_admin_listener() {
        let args = sidecar_args(
            &config(None),
            NetworkType::Testnet,
            12029,
            &LaunchNames {
                mine_address: Some("SKL1abc".to_owned()),
                ..LaunchNames::default()
            },
        );
        let at = args.iter().position(|a| a == "--mine-address").unwrap();
        assert_eq!(args[at + 1], OsString::from("SKL1abc"));
        let admin = args.iter().position(|a| a == "--rpc-admin-listen").unwrap();
        assert_eq!(args[admin + 1], OsString::from("none"));
        assert!(
            !args.iter().any(|a| a == "--owning-connection"),
            "the launcher's"
        );
    }

    /// An old settings file that still carries the keep-running key loads:
    /// the key is not a setting any more, and is not an error either.
    #[test]
    fn a_settings_file_with_the_retired_key_still_loads() {
        let config: DaemonConfig =
            serde_json::from_str(r#"{"keep_running_on_exit": true, "data_dir": "/d"}"#).unwrap();
        assert_eq!(config.data_dir.as_deref(), Some("/d"));
    }

    /// A test's own directory, removed when it goes.
    #[cfg(unix)]
    struct TestDir(PathBuf);

    #[cfg(unix)]
    impl TestDir {
        fn new(what: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let dir = std::env::temp_dir().join(format!(
                "shekyl-gui-{what}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(dir.join("s")).unwrap();
            std::fs::set_permissions(dir.join("s"), std::fs::Permissions::from_mode(0o700))
                .unwrap();
            Self(dir)
        }
    }

    #[cfg(unix)]
    impl Drop for TestDir {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    /// The engine's way to the node on this computer is open only while the
    /// node there is the wallet's own. Before the start it is closed; with
    /// nothing at "this computer" and no node in the build, the wallet
    /// starts nothing, says so, and the way stays closed, with the same
    /// words. Red edit: close the gate only for `Found` and `Refused`, as it
    /// first was; the engine could then dial whatever listens at the port.
    /// Unix: the test names its own socket directory, which Windows refuses.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_engine_s_way_is_closed_unless_the_node_is_the_wallet_s_own() {
        let dir = TestDir::new("gate");
        let gate = NodeGate::default();
        let env = LaunchEnv {
            program: None,
            tor_file: None,
            socket_dir: Some(dir.0.join("s")),
            rpc_port: None,
            extra_args: Vec::new(),
        };
        let state = crate::state::AppState {
            node_gate: gate.clone(),
            ..crate::state::AppState::new()
        };
        assert!(state.engine_base().await.is_ok(), "no manager yet");
        let dm = DaemonManager::new(dir.0.join("settings"), env, gate);
        assert_eq!(
            state.engine_base().await.err().as_deref(),
            Some(NODE_NOT_STARTED),
            "closed before the start"
        );
        assert_eq!(dm.status().await.state, "idle");

        dm.start(NetworkType::Testnet).await;
        let status = dm.status().await;
        assert_eq!(status.state, "unavailable", "{status:?}");
        let words = status.message.expect("why");
        assert_eq!(state.engine_base().await.err(), Some(words.clone()));
        assert_eq!(dm.own_target().await.err(), Some(words));
        assert!(!status.can_force_stop);

        // A node address a person typed is theirs: the gate is about the
        // node on this computer alone.
        *state.typed_node.write().await = true;
        assert!(state.engine_base().await.is_ok());
    }
}
