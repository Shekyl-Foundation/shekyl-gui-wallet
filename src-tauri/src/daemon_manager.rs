// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use shekyl_rpc_client::wire::node::{MineStart, Threads};
use shekyl_rpc_client::wire::MineStatus;
use shekyl_rpc_transport::local::{LocalFault, LocalTarget};
use shekyl_rpc_types::kinds::{Count, NoParams};
use tokio::sync::{watch, RwLock};

use crate::node_launch::{self, Launch, Life, OwnNode};
use crate::state::{NetworkType, NodeGate};

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
pub(crate) fn data_dir_for(root: &Path, network: NetworkType) -> PathBuf {
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
    /// A test's own instance (`--instance`, Windows; see [`test_instance`]).
    /// The wallet names none.
    #[cfg(test)]
    pub instance: Option<String>,
}

/// The node's command line, less the owning connection the launcher adds.
///
/// The daemon's grammar (shekyl-core `docs/design/INPUT_GRAMMAR.md` §5): the
/// GUI's network; no admin listener; no inbound clearnet listener unless
/// the person turned it on (`clearnet-listen none`: the node only dials out
/// on clearnet); one view listener on loopback at
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
    // The wallet's node dials out and takes no inbound clearnet connection
    // unless the person turned it on (Rick, 2026-11-17); on, the daemon's
    // own default listener on the network's peer port is used.
    if !config.clearnet_inbound {
        args.push("--clearnet-listen".into());
        args.push("none".into());
    }
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
    #[cfg(test)]
    if let Some(instance) = &names.instance {
        args.push("--instance".into());
        args.push(instance.into());
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
    /// Whether the node accepts inbound connections over clearnet. Off by
    /// default: the wallet's node dials out and takes none (Rick,
    /// 2026-11-17). On, it listens on its network's peer port.
    #[serde(default)]
    pub clearnet_inbound: bool,
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
    /// The found node took its stop, and something still holds the data
    /// directory. The wallet's own node is started again at growing
    /// intervals until it starts or a person ends the wait.
    Waiting(String),
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
            Self::Waiting(_) => "waiting",
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
    /// `idle`, `starting`, `running`, `stopping`, `waiting`, `found`,
    /// `refused`, `failed` or `unavailable`.
    pub state: &'static str,
    /// The state in words, where there is something to say.
    pub message: Option<String>,
    /// The network the node runs, or is to run.
    pub network: &'static str,
    /// Whether a force stop has a node to act on: one this wallet started.
    pub can_force_stop: bool,
    /// Whether the wallet is waiting on a node it did not start, a wait a
    /// person can end. Ending it kills nothing.
    pub can_stop_waiting: bool,
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
    /// A test's own instance (see [`LaunchNames`]).
    #[cfg(test)]
    pub instance: Option<String>,
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
            instance: None,
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
    /// The wallet is waiting on a node it did not start: for a found node
    /// to be gone, or for the data directory it held.
    waits_on_another: bool,
    /// Why the node is idle after a person ended such a wait. A start that
    /// follows waits out a held data directory again.
    left_waiting: Option<String>,
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
    /// A person's "Stop waiting", read by the flow that waits.
    stop_waiting: watch::Sender<bool>,
    /// Every network a node was launched on, in order, for a test that
    /// checks no node ran on another network first.
    #[cfg(test)]
    launched: std::sync::Mutex<Vec<NetworkType>>,
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
                waits_on_another: false,
                left_waiting: None,
            }),
            config: RwLock::new(config),
            config_dir,
            gate,
            stop_waiting: watch::channel(false).0,
            #[cfg(test)]
            launched: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// "This computer" on `network`: the owner's leg of this account's node.
    fn target(&self, network: NetworkType) -> Result<LocalTarget, LocalFault> {
        #[cfg(test)]
        if self.env.socket_dir.is_some() || self.env.instance.is_some() {
            return LocalTarget::resolve(
                network.network(),
                &test_instance_of(self.env.instance.as_deref()),
                self.env.socket_dir.as_deref(),
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
            NodeState::Waiting(words)
            | NodeState::Refused(words)
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
            NodeState::Waiting(words)
            | NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => Some(words.clone()),
        });
        if state != NodeState::Idle {
            inner.left_waiting = None;
        }
        inner.state = state;
    }

    /// The node said it is ready: `starting`, or `waiting` on a start
    /// retried, becomes `running`, and the engine's way opens.
    fn now_running(&self, inner: &mut Inner) {
        if matches!(inner.state, NodeState::Starting | NodeState::Waiting(_)) {
            inner.state = NodeState::Running;
            self.gate.set(None);
            tracing::info!("the wallet's node is ready");
        }
    }

    /// Start the node on `network`.
    pub async fn start(self: &Arc<Self>, network: NetworkType) {
        let _lifecycle = self.lifecycle.lock().await;
        self.inner.write().await.network = network;
        self.start_locked().await;
    }

    /// Start the wallet's own node again after it failed or was stopped,
    /// or after a person ended a wait for it. After such a wait the start
    /// waits out a held data directory again, as the replacement did.
    pub async fn start_again(self: &Arc<Self>) {
        let _lifecycle = self.lifecycle.lock().await;
        let (has_node, waited) = {
            let inner = self.inner.read().await;
            (inner.node.is_some(), inner.left_waiting.is_some())
        };
        if has_node {
            return;
        }
        if waited {
            self.stop_waiting.send_replace(false);
            self.inner.write().await.waits_on_another = true;
            self.start_waiting_locked(true).await;
            self.inner.write().await.waits_on_another = false;
        } else {
            self.start_locked().await;
        }
    }

    /// One target, never a ladder (core `RPC_CHANNEL.md` RT-15): dial "this
    /// computer" first. Nothing there, and the wallet starts its own node.
    /// A node of this account there is found and left alone. Anything else
    /// there is a hard stop.
    async fn start_locked(self: &Arc<Self>) {
        self.start_waiting_locked(false).await;
    }

    /// [`Self::start_locked`], and with `wait_out_lock` the launch waits
    /// out a held data directory ([`Self::launch_waiting_locked`]).
    async fn start_waiting_locked(self: &Arc<Self>, wait_out_lock: bool) {
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
                if wait_out_lock {
                    drop(self.launch_waiting_locked().await);
                } else {
                    self.launch_locked(false).await;
                }
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

    /// Launch the wallet's own node and follow its life. A `quiet` launch
    /// is a start retried while the state is `waiting`: the state stays
    /// there until the node says it is ready.
    async fn launch_locked(self: &Arc<Self>, quiet: bool) {
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
            #[cfg(test)]
            instance: self.env.instance.clone(),
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
        #[cfg(test)]
        self.launched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(network);
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
        if !quiet {
            self.set_state(NodeState::Starting).await;
        }
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
                    if inner.launch == id {
                        self.now_running(&mut inner);
                    }
                }
                Life::Gone { refusal } => {
                    let mut inner = self.inner.write().await;
                    if inner.launch == id {
                        inner.node = None;
                        // Only a node that was starting or running has
                        // failed by going. A stop this wallet asked for, a
                        // start retried while waiting, and a wait a person
                        // ended leave the state to their flows.
                        if matches!(inner.state, NodeState::Starting | NodeState::Running) {
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
                NodeState::Waiting(words)
                | NodeState::Refused(words)
                | NodeState::Unavailable(words) => return Err(words.clone()),
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

    /// A wallet with `address` was opened. The node is started again for
    /// it only while its miner is running and pays another address (Rick,
    /// 2026-11-17): the miner then goes on, with the threads it had, paying
    /// the open wallet. A node that is not mining is left alone;
    /// [`Self::mine_to`] gives it the address when mining is started.
    pub async fn follow_wallet(self: &Arc<Self>, address: &str) -> Result<(), String> {
        let pays_another = self
            .inner
            .read()
            .await
            .mine_address
            .as_deref()
            .is_some_and(|paid| paid != address);
        if !pays_another {
            return Ok(());
        }
        let Some(threads) = self.running_miner().await? else {
            return Ok(());
        };
        self.mine_to(address).await?;
        self.start_miner(threads).await
    }

    /// The threads of the wallet's own node's miner while it runs. `None`
    /// when it does not, and when the wallet has no node of its own ready:
    /// nothing of its is mining then.
    async fn running_miner(&self) -> Result<Option<u64>, String> {
        let Ok(leg) = self.own_target().await else {
            return Ok(None);
        };
        let miner = tokio::task::spawn_blocking(move || leg.call::<MineStatus>(&NoParams))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("The node's miner could not be read: {e}"))?;
        Ok(miner.active.then_some(miner.threads.0))
    }

    /// Start the miner of the node just started again, with the threads it
    /// had before.
    async fn start_miner(&self, threads: u64) -> Result<(), String> {
        let leg = self.own_target().await?;
        let threads = Threads::Count(Count(threads));
        tokio::task::spawn_blocking(move || {
            leg.call::<shekyl_rpc_client::wire::MineStart>(&MineStart { threads })
        })
        .await
        .map_err(|e| e.to_string())?
        .map(drop)
        .map_err(|e| format!("The miner did not start again: {e}"))
    }

    /// Accept inbound clearnet connections or not (`--clearnet-listen`):
    /// kept in the node's settings and passed at its next start, so the
    /// node is started again with it. Off is the wallet's default (Rick,
    /// 2026-11-17); on, the node listens on its network's peer port.
    pub async fn set_clearnet_inbound(self: &Arc<Self>, on: bool) -> Result<(), String> {
        let mut config = self.config().await;
        if config.clearnet_inbound == on {
            return Ok(());
        }
        tracing::info!(
            on,
            "the node's inbound clearnet listener changes; the node is started again"
        );
        config.clearnet_inbound = on;
        self.update_config(config).await?;
        self.relaunch_for_settings().await
    }

    /// The node's settings changed (the inbound clearnet listener, a Tor
    /// fix taken or put back): the
    /// daemon reads them only at its start, so a node this wallet started
    /// is stopped and started again with them, and a miner that was
    /// running goes on. A node that had failed to start is started. A node
    /// of this account the wallet found is left alone: it is not the
    /// wallet's, and the settings are for the wallet's own.
    pub async fn relaunch_for_settings(self: &Arc<Self>) -> Result<(), String> {
        let miner = self.running_miner().await?;
        {
            let _lifecycle = self.lifecycle.lock().await;
            {
                let inner = self.inner.read().await;
                match &inner.state {
                    NodeState::Found => return Err(found_words(inner.network)),
                    NodeState::Refused(words) | NodeState::Unavailable(words) => {
                        return Err(words.clone())
                    }
                    _ => {}
                }
            }
            self.stop_own_locked().await;
            self.start_locked().await;
            self.ready_or_words().await?;
        }
        match miner {
            Some(threads) => self.start_miner(threads).await,
            None => Ok(()),
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
                    self.now_running(&mut *self.inner.write().await);
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
            NodeState::Waiting(words)
            | NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => words.clone(),
            _ => "The wallet's node did not start.".to_owned(),
        })
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
            NodeState::Waiting(words)
            | NodeState::Refused(words)
            | NodeState::Failed(words)
            | NodeState::Unavailable(words) => Some(words.clone()),
            NodeState::Idle => inner.left_waiting.clone(),
            NodeState::Starting | NodeState::Running | NodeState::Stopping => None,
        };
        DaemonStatus {
            state: inner.state.keyword(),
            message,
            network: inner.network.as_str(),
            can_force_stop: inner.node.is_some(),
            can_stop_waiting: inner.waits_on_another,
        }
    }

    /// Every network a node was launched on, in order.
    #[cfg(test)]
    pub(crate) fn launched(&self) -> Vec<NetworkType> {
        self.launched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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

mod replace;

/// A test's own instance name, from its port: on Windows the node's
/// rendezvous cannot be moved out of the account's
/// `%LOCALAPPDATA%\Shekyl\rendezvous` (core refuses `rpc-socket-dir` there
/// and takes the folder from the system, not the environment), so a test's
/// node is a named instance of its own there instead, and never the
/// account's default one. Its record is removed when the test's manager
/// goes ([`DaemonManager`]'s `Drop`), and by the older tests' guard.
#[cfg(test)]
pub(crate) fn test_instance(port: u16) -> String {
    format!("t{port}")
}

/// The instance `name` names, or the default one.
#[cfg(test)]
pub(crate) fn test_instance_of(name: Option<&str>) -> shekyl_rpc_rendezvous::Instance {
    name.map_or(shekyl_rpc_rendezvous::Instance::Default, |name| {
        shekyl_rpc_rendezvous::Instance::Named(
            shekyl_rpc_rendezvous::InstanceName::parse(name).expect("a test's instance name"),
        )
    })
}

/// Remove a test instance's rendezvous records, one per network. A node
/// removes its own at a clean stop; a killed one cannot.
#[cfg(test)]
pub(crate) fn remove_test_records(instance: &str) {
    let mut networks: Vec<shekyl_engine_core::Network> =
        NetworkType::ALL.iter().map(|n| n.network()).collect();
    networks.push(shekyl_engine_core::Network::Regtest);
    for network in networks {
        if let Ok(target) = LocalTarget::resolve(network, &test_instance_of(Some(instance)), None) {
            drop(std::fs::remove_file(target.path()));
        }
    }
}

#[cfg(test)]
impl Drop for DaemonManager {
    fn drop(&mut self) {
        if let Some(instance) = &self.env.instance {
            remove_test_records(instance);
        }
    }
}

#[cfg(test)]
mod tests;
