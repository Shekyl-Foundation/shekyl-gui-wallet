// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Verifying daemon client for Engine sessions (VC-4).
//!
//! Construction plus the identity handshake: [`DaemonClient::verifying`]
//! against this wallet's network — an exact identity match, with no
//! policy to relax it — then one Engine RPC so a mismatch is known
//! **before** `Engine::create` writes a wallet file.
//! Transport failures are not stored and are not fatal — an unreachable
//! daemon still lets create/open proceed (offline). The session owns when
//! the client is attached.

use std::path::PathBuf;
use std::sync::Arc;

use shekyl_engine_core::{DaemonClient, DaemonExpectation, Network};
use shekyl_rpc_client::{DaemonFault, Rpc};
use shekyl_rpc_transport::channel::RemoteNode;
use shekyl_rpc_transport::local::LocalTarget;
use shekyl_rpc_transport::{DaemonTarget, DaemonTransport};
use shekyl_wallet_contract::error::from_daemon_rpc_error;

use crate::daemon_manager::DaemonManager;
use crate::state::AppState;

/// This computer's account rendezvous, as core names it.
///
/// `key_file` overrides the account key. `None` selects the account default
/// (`rpc-client.key` under the account root) when resolution falls through
/// to a service channel. The owner socket wins when its path exists, and
/// the key is not read. This is the one construction of
/// [`DaemonTarget::ThisComputer`]: the engine, the manager, and the app
/// state all name this computer through it.
#[must_use]
pub(crate) fn this_computer(network: Network, key_file: Option<PathBuf>) -> DaemonTarget {
    DaemonTarget::ThisComputer { network, key_file }
}

/// The node the wallet engine reads (core RT-W10 step 5).
///
/// One variant per door core names in [`DaemonTarget`]. [`Self::ThisComputer`]
/// is the account rendezvous the wallet starts: the manager latches a failed
/// peer check there, and a test may substitute its own socket. [`Self::Owner`]
/// is an already-resolved owner socket (a harness, or a test node's leg).
/// [`Self::Channel`] is an enrolled channel; the desktop wallet does not build
/// one yet, and the variant is the door so a later caller is not folded into
/// another. [`Self::Address`] is a plaintext address a person typed.
///
/// The transport is the RPC door. It does not choose a proof system, so the
/// same four doors hold through the mining era and the lattice transition.
#[derive(Clone)]
pub(crate) enum EngineNode {
    /// This computer's account rendezvous. `key_file` is the target's, kept
    /// so a later override is not dropped when the target is rebuilt.
    ThisComputer {
        /// The manager that starts this node and latches a failed peer check.
        manager: Arc<DaemonManager>,
        /// Account key file. `None` is the account default; see [`this_computer`].
        key_file: Option<PathBuf>,
    },
    /// An already-resolved owner socket. Dial this rendezvous. The manager's
    /// latch does not apply: the socket is not the account rendezvous the
    /// manager watches.
    Owner(LocalTarget),
    /// An enrolled channel. Dial this target. The latch does not apply.
    Channel {
        /// The channel's network, which is the target's, not the session's.
        network: Network,
        /// The endpoint and the operator's bundle.
        node: RemoteNode,
        /// Account key file. `None` is the account default.
        key_file: Option<PathBuf>,
    },
    /// A plaintext address a person typed. Dial it. The latch does not apply,
    /// loopback included: an address they typed is theirs.
    Address {
        /// The daemon's base URL; may carry digest credentials.
        url: String,
        /// The SOCKS5h proxy; `None` is direct.
        proxy: Option<String>,
    },
}

impl std::fmt::Debug for EngineNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // The manager is a process handle. The key file is the part of
            // the door a log can show.
            Self::ThisComputer { key_file, .. } => f
                .debug_struct("ThisComputer")
                .field("key_file", key_file)
                .finish_non_exhaustive(),
            Self::Owner(target) => f.debug_tuple("Owner").field(target).finish(),
            Self::Channel {
                network,
                node,
                key_file,
            } => f
                .debug_struct("Channel")
                .field("network", network)
                .field("node", node)
                .field("key_file", key_file)
                .finish(),
            // Core's `Debug` redacts digest credentials in the URL.
            Self::Address { url, proxy } => f
                .debug_tuple("Address")
                .field(&DaemonTarget::Address {
                    url: url.clone(),
                    proxy: proxy.clone(),
                })
                .finish(),
        }
    }
}

impl EngineNode {
    /// `door` becomes the engine's node. `manager` is attached only for this
    /// computer: that is the rendezvous the manager starts and watches.
    /// An owner socket, a channel, and a typed address keep the target
    /// itself, and the manager is not latched for them.
    pub(crate) fn from_target(door: DaemonTarget, manager: &Arc<DaemonManager>) -> Self {
        match door {
            DaemonTarget::ThisComputer { key_file, .. } => Self::ThisComputer {
                manager: Arc::clone(manager),
                key_file,
            },
            DaemonTarget::Owner(owner) => Self::Owner(owner),
            DaemonTarget::Channel {
                network,
                node,
                key_file,
            } => Self::Channel {
                network,
                node,
                key_file,
            },
            DaemonTarget::Address { url, proxy } => Self::Address { url, proxy },
        }
    }

    /// The node the wallet is pointed at now. Read inside the session
    /// guard by whatever opens a wallet (see [`AppState::engine`]).
    pub(crate) async fn of(state: &AppState, manager: &Arc<DaemonManager>) -> Self {
        Self::from_target(state.engine_target().await, manager)
    }

    /// The target core dials, with no latch applied.
    ///
    /// For [`Self::ThisComputer`], `session_network` is the session's
    /// network and `key_file` is the target's. The other doors return the
    /// target they hold: a channel keeps its own network, and the session
    /// argument does not rewrite it.
    fn target(&self, session_network: Network) -> DaemonTarget {
        match self {
            Self::ThisComputer { key_file, .. } => this_computer(session_network, key_file.clone()),
            Self::Owner(owner) => DaemonTarget::Owner(owner.clone()),
            Self::Channel {
                network,
                node,
                key_file,
            } => DaemonTarget::Channel {
                network: *network,
                node: node.clone(),
                key_file: key_file.clone(),
            },
            Self::Address { url, proxy } => DaemonTarget::Address {
                url: url.clone(),
                proxy: proxy.clone(),
            },
        }
    }

    /// Core's target for this node, for a call core dials itself (the
    /// stake's body-store fill). Refused while a failed peer check at this
    /// computer is latched. An owner socket, a channel, and a typed address
    /// are not that rendezvous, so the latch does not stand in front of them.
    pub(crate) async fn core_target(
        &self,
        session_network: Network,
    ) -> Result<DaemonTarget, String> {
        if let Self::ThisComputer { manager, .. } = self {
            if let Some(words) = manager.refusal().await {
                return Err(words);
            }
        }
        Ok(self.target(session_network))
    }
}

/// Core's target for a call core dials itself (`unstake`). The same door
/// as [`EngineNode::core_target`]: this computer is refused while a failed
/// peer check is latched, and every other door is handed back as named.
pub(crate) async fn core_target(
    state: &AppState,
    manager: &Arc<DaemonManager>,
) -> Result<DaemonTarget, String> {
    let session_network = state.network.read().await.network();
    EngineNode::of(state, manager)
        .await
        .core_target(session_network)
        .await
}

/// Build the engine's verified client for `node`. A failed peer check or a
/// service at "this computer" refuses here, in core's words, and nothing
/// is written for a wallet against such a node. The failed check is
/// latched in the node's manager as well as in the client, so the notice
/// says it and no later client dials that address (Rick, 2026-11-18:
/// latched as a refusal, never retried).
pub(crate) async fn make_daemon(
    node: &EngineNode,
    network: Network,
) -> Result<DaemonClient, String> {
    let rpc: DaemonTransport = match node {
        EngineNode::ThisComputer { manager, key_file } => {
            manager.engine_transport(network, key_file.clone()).await?
        }
        direct => direct
            .target(network)
            .connect(None)
            .await
            .map_err(|e| format!("daemon unreachable: {e}"))?,
    };
    let daemon = DaemonClient::verifying(rpc, DaemonExpectation { network });
    // First Engine RPC runs the four-axis handshake. Identity mismatch
    // refuses here so create/restore never write a file the person cannot
    // recover (the phrase is create-once; there is no seed-returning
    // command). The verdict is read from its type; the refusal is said in
    // the wallet contract's words for its axis. Any other failure — an
    // outage above all — is not a mismatch, and create/open proceed offline.
    match daemon.get_height().await {
        Err(e) if matches!(e.fault(), DaemonFault::Identity(_)) => {
            Err(from_daemon_rpc_error(&e).message())
        }
        Err(e) => match e.fault() {
            DaemonFault::PeerCheck => {
                let words = crate::daemon_rpc::said(&e, "the node");
                if let EngineNode::ThisComputer { manager, .. } = node {
                    manager.refuse(words.clone()).await;
                }
                Err(words)
            }
            DaemonFault::Service(_) => Err(crate::daemon_rpc::said(&e, "the node")),
            _ => Ok(daemon),
        },
        Ok(_) => Ok(daemon),
    }
}
