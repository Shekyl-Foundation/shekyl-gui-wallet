// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use shekyl_rpc_transport::local::{LocalFault, LocalTarget};
use tokio::sync::{watch, RwLock};

use crate::node_launch::{self, Launch, Life, OwnNode};
use crate::state::NetworkType;

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
/// GUI's network; no admin listener; no view listener (the daemon's default:
/// the wallet and its engine read the node over its owner's leg, which the
/// daemon serves without being asked; Rick, 2026-11-18); no inbound
/// clearnet listener unless the person turned it on (`clearnet-listen
/// none`: the node only dials out on clearnet); the data directory when
/// configured.
///
/// `--tor-file` names the bundled Tor explicitly, one mechanism on every
/// platform (GUI_SYNC_LANE.md §4, GS-1). The daemon checks that file
/// against its pin at every spawn; a file it cannot pin is a warning and
/// the node holds its own transactions until Tor is up (core TB-2), never
/// a refusal to start.
pub(crate) fn sidecar_args(
    config: &DaemonConfig,
    network: NetworkType,
    names: &LaunchNames,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--network".into(),
        network.as_str().into(),
        "--rpc-admin-listen".into(),
        "none".into(),
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
    /// "This computer"'s node on this network runs as a service, which the
    /// wallet reaches only over the channel from RT-W12; until then over
    /// its view listener, when it has one, as a typed address. The wallet
    /// starts no node of its own beside it (Rick, 2026-11-18).
    Service(Option<std::net::SocketAddr>),
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
            Self::Service(_) => "service",
        }
    }

    /// The state in words, where it has something to say.
    fn words(&self, network: NetworkType) -> Option<String> {
        match self {
            Self::Found => Some(found_words(network)),
            Self::Service(view) => Some(shekyl_rpc_client::service_refusal(*view)),
            Self::Waiting(words)
            | Self::Refused(words)
            | Self::Failed(words)
            | Self::Unavailable(words) => Some(words.clone()),
            Self::Idle | Self::Starting | Self::Running | Self::Stopping => None,
        }
    }
}

/// What stands at "this computer" when no node of this account answers
/// there: nothing, so the wallet may start its own node; or something the
/// wallet starts no node beside, with the state that says what.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Standing {
    /// No node, and no record of one that still runs.
    Nothing,
    /// A failed check, a service, or a fault the wallet cannot read past.
    Blocked(NodeState),
}

/// Read a failed dial of "this computer". Exhaustive, so a fault core adds
/// does not compile until it is placed: only the faults that say no node
/// is there let the wallet start one (one target, never a ladder: core
/// `RPC_CHANNEL.md` RT-15).
pub(crate) fn standing(fault: LocalFault) -> Standing {
    match fault {
        LocalFault::NoPlace { .. } | LocalFault::NotRunning { .. } | LocalFault::Stale { .. } => {
            Standing::Nothing
        }
        LocalFault::Service { view, .. } => Standing::Blocked(NodeState::Service(view)),
        fault @ LocalFault::PeerCheck(_) => {
            Standing::Blocked(NodeState::Refused(fault.to_string()))
        }
        fault @ (LocalFault::OtherSession | LocalFault::Unreachable(_)) => {
            Standing::Blocked(NodeState::Failed(fault.to_string()))
        }
    }
}

/// The node's state for the page.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct DaemonStatus {
    /// `idle`, `starting`, `running`, `stopping`, `waiting`, `found`,
    /// `service`, `refused`, `failed` or `unavailable`.
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
    /// The miner could not be moved to the open wallet's address: it was
    /// stopped, or may still pay the wallet open before. Said whatever the
    /// node's state, so it is seen without opening the Mining page.
    pub payout_fault: Option<String>,
    /// A service node's view listener, while "this computer" is a service
    /// with one: the page offers "Use the service's node".
    pub service_view: Option<String>,
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
    /// A test's own service rendezvous directory, where it stages a
    /// service's record. The wallet names none: core's "this computer"
    /// reads the platform's.
    #[cfg(test)]
    pub service_rendezvous: Option<PathBuf>,
    /// A test's own socket directory (see [`LaunchNames`]).
    #[cfg(test)]
    pub socket_dir: Option<PathBuf>,
    /// A test's own instance (see [`LaunchNames`]).
    #[cfg(test)]
    pub instance: Option<String>,
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
            service_rendezvous: None,
            #[cfg(test)]
            socket_dir: None,
            #[cfg(test)]
            instance: None,
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
    /// The arguments that node was started with: the settings it runs
    /// with, which may not be the settings last saved.
    node_args: Vec<OsString>,
    /// Counts launches, so a launch's follower never writes over a later
    /// launch's state.
    launch: u64,
    /// The wallet is waiting on a node it did not start: for a found node
    /// to be gone, or for the data directory it held.
    waits_on_another: bool,
    /// Why the node is idle after a person ended such a wait. A start that
    /// follows waits out a held data directory again.
    left_waiting: Option<String>,
    /// The open wallet's address, which a running miner is moved to.
    open_wallet: Option<String>,
    /// Why the miner was stopped, or may still pay a wallet no longer open,
    /// after it could not be moved to the open wallet ([`payout`]).
    payout_fault: Option<String>,
}

pub struct DaemonManager {
    env: LaunchEnv,
    /// Held across a whole start, relaunch or replacement, so two of them
    /// never interleave.
    lifecycle: tokio::sync::Mutex<()>,
    inner: RwLock<Inner>,
    config: RwLock<DaemonConfig>,
    config_dir: PathBuf,
    /// A person's "Stop waiting", read by the flow that waits.
    stop_waiting: watch::Sender<bool>,
    /// Every network a node was launched on, in order, for a test that
    /// checks no node ran on another network first.
    #[cfg(test)]
    launched: std::sync::Mutex<Vec<NetworkType>>,
}

impl DaemonManager {
    pub(crate) fn new(config_dir: PathBuf, env: LaunchEnv) -> Arc<Self> {
        let config = DaemonConfig::load(&config_dir);
        Arc::new(Self {
            env,
            lifecycle: tokio::sync::Mutex::new(()),
            inner: RwLock::new(Inner {
                network: NetworkType::default(),
                mine_address: None,
                state: NodeState::Idle,
                node: None,
                node_args: Vec::new(),
                launch: 0,
                waits_on_another: false,
                left_waiting: None,
                open_wallet: None,
                payout_fault: None,
            }),
            config: RwLock::new(config),
            config_dir,
            stop_waiting: watch::channel(false).0,
            #[cfg(test)]
            launched: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// "This computer" on `network`, as core defines it: this account's
    /// node over its owner's leg, or the machine's service, said as one. A
    /// test names its own directories instead.
    fn target(&self, network: NetworkType) -> LocalTarget {
        #[cfg(test)]
        if let Some(target) = self.test_target(network.network()) {
            return target;
        }
        LocalTarget::this_computer(network.network())
    }

    /// A test's own directories in place of "this computer", when it
    /// named any.
    #[cfg(test)]
    fn test_target(&self, network: shekyl_engine_core::Network) -> Option<LocalTarget> {
        (self.env.socket_dir.is_some()
            || self.env.instance.is_some()
            || self.env.service_rendezvous.is_some())
        .then(|| {
            LocalTarget::test_override(
                network,
                &test_instance_of(self.env.instance.as_deref()),
                self.env.socket_dir.as_deref(),
                self.env.service_rendezvous.as_deref(),
            )
        })
    }

    /// The owner's leg of the wallet's own node, for a call: only while the
    /// node this wallet started is ready. Otherwise the state, in words.
    pub(crate) async fn own_target(&self) -> Result<LocalTarget, String> {
        let inner = self.inner.read().await;
        match &inner.state {
            NodeState::Running => Ok(self.target(inner.network)),
            NodeState::Idle => Err(NODE_NOT_STARTED.to_owned()),
            NodeState::Starting => Err("The wallet's node is starting.".to_owned()),
            NodeState::Stopping => Err("The node is stopping.".to_owned()),
            state @ (NodeState::Found
            | NodeState::Service(_)
            | NodeState::Waiting(_)
            | NodeState::Refused(_)
            | NodeState::Failed(_)
            | NodeState::Unavailable(_)) => Err(state.words(inner.network).unwrap_or_default()),
        }
    }

    /// The service node's view listener, while "this computer" is a
    /// service with one: what "Use the service's node" points the wallet at.
    pub(crate) async fn service_view(&self) -> Option<std::net::SocketAddr> {
        match self.inner.read().await.state {
            NodeState::Service(view) => view,
            _ => None,
        }
    }

    /// Call `M` on the owner's leg of the wallet's own node, while it is
    /// ready. A failed peer check there is latched: something other than
    /// this wallet's node answers at its address, so the state becomes
    /// `refused`, the engine's way closes, and nothing is dialled again
    /// until the person starts the node again. The wallet's own node is not
    /// killed: it is still the wallet's, by its owning connection.
    pub(crate) async fn own_call<M: shekyl_rpc_client::RpcMethod>(
        &self,
        request: M::Request,
        what: &str,
    ) -> Result<M::Reply, String>
    where
        M::Request: Send + 'static,
        M::Reply: Send + 'static,
    {
        let target = self.own_target().await?;
        // The owner's leg is a blocking dial and exchange, under its own
        // deadline (`shekyl_rpc_transport::local::CALL_WITHIN`).
        let reply = tokio::task::spawn_blocking(move || target.call::<M>(&request))
            .await
            .map_err(|e| format!("Could not read {what} from the node: {e}"))?;
        match reply {
            Ok(reply) => Ok(reply),
            Err(e) => {
                let words = crate::daemon_rpc::said_local(&e, what);
                if let shekyl_rpc_transport::JsonRpcError::Transport(
                    fault @ LocalFault::PeerCheck(_),
                ) = &e
                {
                    self.refuse(fault.to_string()).await;
                }
                Err(words)
            }
        }
    }

    async fn set_state(&self, state: NodeState) {
        let mut inner = self.inner.write().await;
        if state != NodeState::Idle {
            inner.left_waiting = None;
        }
        inner.state = state;
    }

    /// The node said it is ready: `starting`, or `waiting` on a start
    /// retried, becomes `running`.
    fn now_running(&self, inner: &mut Inner) {
        if matches!(inner.state, NodeState::Starting | NodeState::Waiting(_)) {
            inner.state = NodeState::Running;
            tracing::info!("the wallet's node is ready");
        }
    }

    /// The node the state holds is gone, with `words`. Only a node that
    /// was starting or running has failed by going: a stop this wallet
    /// asked for, a start retried while waiting, and a wait a person ended
    /// leave the state to their flows.
    fn now_gone(inner: &mut Inner, words: String) {
        inner.node = None;
        if matches!(inner.state, NodeState::Starting | NodeState::Running) {
            tracing::warn!("the wallet's node is gone: {words}");
            inner.state = NodeState::Failed(words);
        }
    }

    /// Start the node on `network`.
    pub async fn start(self: &Arc<Self>, network: NetworkType) {
        let _lifecycle = self.lifecycle.lock().await;
        self.inner.write().await.network = network;
        drop(self.start_locked().await);
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
            drop(self.start_waiting_locked(true).await);
            self.inner.write().await.waits_on_another = false;
        } else {
            drop(self.start_locked().await);
        }
    }

    /// One target, never a ladder (core `RPC_CHANNEL.md` RT-15): dial "this
    /// computer" first. Nothing there, and the wallet starts its own node.
    /// A node of this account there is found and left alone. Anything else
    /// there is a hard stop.
    ///
    /// Returns the node it launched, for the flow that waits on it.
    async fn start_locked(self: &Arc<Self>) -> Option<Arc<OwnNode>> {
        self.start_waiting_locked(false).await
    }

    /// [`Self::start_locked`], and with `wait_out_lock` the launch waits
    /// out a held data directory ([`Self::launch_waiting_locked`]).
    async fn start_waiting_locked(self: &Arc<Self>, wait_out_lock: bool) -> Option<Arc<OwnNode>> {
        let network = self.inner.read().await.network;
        match self.dial(network).await {
            Ok(()) => {
                tracing::info!(
                    network = network.as_str(),
                    "a node of this account is already running; it is not used"
                );
                self.set_state(NodeState::Found).await;
                None
            }
            Err(fault) => match standing(fault) {
                Standing::Nothing if wait_out_lock => {
                    // That flow waits on its own launches.
                    drop(self.launch_waiting_locked().await);
                    None
                }
                Standing::Nothing => self.launch_locked(false).await,
                Standing::Blocked(state) => {
                    self.block(network, state).await;
                    None
                }
            },
        }
    }

    /// Something stands at "this computer" that the wallet starts no node
    /// beside: the state says what, and its words are returned.
    async fn block(&self, network: NetworkType, state: NodeState) -> String {
        let words = state.words(network).unwrap_or_default();
        match &state {
            NodeState::Refused(_) => {
                tracing::error!("the node's local address failed its check: {words}");
            }
            _ => tracing::info!(network = network.as_str(), "no node is started: {words}"),
        }
        self.set_state(state).await;
        words
    }

    /// Dial "this computer" on `network`, and close the connection.
    async fn dial(&self, network: NetworkType) -> Result<(), LocalFault> {
        let target = self.target(network);
        tokio::task::spawn_blocking(move || target.dial().map(drop))
            .await
            .unwrap_or_else(|e| Err(LocalFault::Unreachable(e.to_string())))
    }

    /// The network and the arguments the next launch uses: the saved
    /// settings, the network, and the miner's address, as they stand now.
    async fn launch_args(&self) -> (NetworkType, Vec<OsString>) {
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
        #[allow(unused_mut, reason = "a test's launch adds its own arguments")]
        let mut args = sidecar_args(&config, network, &names);
        #[cfg(test)]
        args.extend(self.env.extra_args.iter().cloned());
        (network, args)
    }

    /// Whether the wallet's node lives and was started with the settings
    /// as they stand now: nothing is left to apply by starting it again.
    async fn runs_the_saved_settings(&self) -> bool {
        let (_, args) = self.launch_args().await;
        let inner = self.inner.read().await;
        inner.node.is_some() && inner.node_args == args
    }

    /// Launch the wallet's own node and follow its life. A `quiet` launch
    /// is a start retried while the state is `waiting`: the state stays
    /// there until the node says it is ready.
    ///
    /// Returns the node launched, so the flow that launched it waits on
    /// that node's own life ([`Self::ready_or_words`]) and never on
    /// whichever node the state holds by then. `None`: nothing started,
    /// and the state says why.
    async fn launch_locked(self: &Arc<Self>, quiet: bool) -> Option<Arc<OwnNode>> {
        let Some(program) = self.env.program.clone() else {
            self.set_state(NodeState::Unavailable(
                "This build of the wallet carries no node to start.".to_owned(),
            ))
            .await;
            return None;
        };
        let (network, args) = self.launch_args().await;
        let launch = Launch {
            program,
            args: args.clone(),
            stderr: self.config_dir.join("shekyld.stderr"),
        };
        if let Err(e) = std::fs::create_dir_all(&self.config_dir) {
            self.set_state(NodeState::Failed(format!(
                "The wallet's settings directory {} could not be made: {e}",
                self.config_dir.display()
            )))
            .await;
            return None;
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
                return None;
            }
            Err(e) => {
                self.set_state(NodeState::Failed(format!(
                    "The node could not be started: {e}"
                )))
                .await;
                return None;
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
            inner.node = Some(Arc::clone(&node));
            inner.node_args = args;
            inner.launch
        };
        if !quiet {
            self.set_state(NodeState::Starting).await;
        }
        let manager = Arc::clone(self);
        tokio::spawn(async move { manager.follow(id, life).await });
        Some(node)
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
                        let words = refusal.unwrap_or_else(|| NODE_STOPPED.to_owned());
                        Self::now_gone(&mut inner, words);
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
        {
            let mut inner = self.inner.write().await;
            inner.network = network;
            // A miner's address belongs to one network, and the new node
            // would refuse the old one at its start; mining on this network
            // sets its own (`mine_to`).
            inner.mine_address = None;
        }
        drop(self.start_locked().await);
    }

    /// Wait for `node`, the one this flow launched, to be ready, or say
    /// why it is not: its own life, never the state's latest node, so a
    /// node that refuses and is gone before this looks still gives its own
    /// words. `None`: the launch did not happen, and the state says why.
    async fn ready_or_words(&self, node: Option<Arc<OwnNode>>) -> Result<(), String> {
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
                // The node's own words, from the read that ended. The
                // follower writes the same state; this does not wait for
                // it, so the state never lags the words this returns.
                Life::Gone { refusal } => {
                    let words = refusal.unwrap_or_else(|| NODE_STOPPED.to_owned());
                    let mut inner = self.inner.write().await;
                    if inner
                        .node
                        .as_ref()
                        .is_some_and(|held| Arc::ptr_eq(held, &node))
                    {
                        Self::now_gone(&mut inner, words.clone());
                    }
                    return Err(words);
                }
                Life::Starting => {}
            }
        }
        let inner = self.inner.read().await;
        Err(inner
            .state
            .words(inner.network)
            .unwrap_or_else(|| "The wallet's node did not start.".to_owned()))
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
            NodeState::Idle => inner.left_waiting.clone(),
            state => state.words(inner.network),
        };
        DaemonStatus {
            service_view: match inner.state {
                NodeState::Service(view) => view.map(|v| v.to_string()),
                _ => None,
            },
            state: inner.state.keyword(),
            message,
            network: inner.network.as_str(),
            can_force_stop: inner.node.is_some(),
            can_stop_waiting: inner.waits_on_another,
            payout_fault: inner.payout_fault.clone(),
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

mod engine;
mod payout;
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
        let target =
            LocalTarget::test_override(network, &test_instance_of(Some(instance)), None, None);
        if let Some(path) = target.path() {
            drop(std::fs::remove_file(path));
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
