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

use std::sync::Arc;

use shekyl_engine_core::{DaemonClient, DaemonExpectation, Network};
use shekyl_rpc_client::{DaemonFault, Rpc};
#[cfg(test)]
use shekyl_rpc_transport::local::LocalRpc;
use shekyl_rpc_transport::{DaemonTarget, DaemonTransport};
use shekyl_wallet_contract::error::from_daemon_rpc_error;

use crate::daemon_manager::DaemonManager;
use crate::state::AppState;

/// The node the wallet engine reads (core RT-W10 step 5; Rick,
/// 2026-11-18): core's one target, "this computer" or a typed address
/// ([`AppState::engine_target`]). A test names its own node's owner's leg
/// directly, since a test node lives in a socket directory or instance of
/// its own, which "this computer" does not name.
#[derive(Clone)]
pub(crate) enum EngineNode {
    /// "This computer", named by the network alone. The node's manager
    /// keeps the one latch for a failed peer check there
    /// ([`DaemonManager::refuse`]): the engine's dial is refused while one
    /// is latched, and latches the one it meets.
    ThisComputer(Arc<DaemonManager>),
    /// The node address a person typed.
    Address(DaemonTarget),
    /// A test node's owner's leg.
    #[cfg(test)]
    Local(shekyl_rpc_transport::local::LocalTarget),
}

impl std::fmt::Debug for EngineNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThisComputer(_) => f.write_str("ThisComputer"),
            Self::Address(target) => f.debug_tuple("Address").field(target).finish(),
            #[cfg(test)]
            Self::Local(target) => f.debug_tuple("Local").field(target).finish(),
        }
    }
}

impl EngineNode {
    /// The node the wallet is pointed at now. Read inside the session
    /// guard by whatever opens a wallet (see [`AppState::engine`]).
    pub(crate) async fn of(state: &AppState, manager: &Arc<DaemonManager>) -> Self {
        match state.engine_target().await {
            DaemonTarget::ThisComputer { .. } | DaemonTarget::Owner(_) => {
                Self::ThisComputer(Arc::clone(manager))
            }
            target @ (DaemonTarget::Address { .. } | DaemonTarget::Channel { .. }) => {
                Self::Address(target)
            }
        }
    }
}

impl EngineNode {
    /// Core's target for this node, for a call core dials itself (the
    /// stake's body-store fill): refused while a failed peer check at "this
    /// computer" is latched, as [`core_target`] is.
    pub(crate) async fn core_target(&self, network: Network) -> Result<DaemonTarget, String> {
        match self {
            Self::ThisComputer(manager) => {
                if let Some(words) = manager.refusal().await {
                    return Err(words);
                }
                Ok(DaemonTarget::ThisComputer {
                    network,
                    key_file: None,
                })
            }
            Self::Address(target) => Ok(target.clone()),
            // A test node's owner's leg is no `DaemonTarget`; the tests that
            // stake refuse before core dials it.
            #[cfg(test)]
            Self::Local(_) => Ok(DaemonTarget::ThisComputer {
                network,
                key_file: None,
            }),
        }
    }
}

/// Core's target for a call core dials itself (`unstake`): the same one
/// target, refused while a failed peer check at "this computer" is
/// latched, so that call does not dial the address either.
pub(crate) async fn core_target(
    state: &AppState,
    manager: &DaemonManager,
) -> Result<DaemonTarget, String> {
    let target = state.engine_target().await;
    if let DaemonTarget::ThisComputer { .. } = target {
        if let Some(words) = manager.refusal().await {
            return Err(words);
        }
    }
    Ok(target)
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
        EngineNode::ThisComputer(manager) => manager.engine_transport(network).await?,
        EngineNode::Address(target) => target
            .connect(None)
            .await
            .map_err(|e| format!("daemon unreachable: {e}"))?,
        #[cfg(test)]
        EngineNode::Local(target) => LocalRpc::new(target.clone()).into(),
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
                if let EngineNode::ThisComputer(manager) = node {
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
