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

use shekyl_engine_core::{DaemonClient, DaemonExpectation, Network};
use shekyl_rpc_client::{DaemonFault, Rpc};
#[cfg(test)]
use shekyl_rpc_transport::local::LocalRpc;
use shekyl_rpc_transport::{DaemonTarget, DaemonTransport};
use shekyl_wallet_contract::error::from_daemon_rpc_error;

/// The node the wallet engine reads (core RT-W10 step 5; Rick,
/// 2026-11-18): core's one target, "this computer" or a typed address
/// ([`crate::state::AppState::engine_target`]). A test names its own node's
/// owner's leg directly, since a test node lives in a socket directory or
/// instance of its own, which "this computer" does not name.
#[derive(Clone, Debug)]
pub(crate) enum EngineNode {
    /// Core's target.
    Target(DaemonTarget),
    /// A test node's owner's leg.
    #[cfg(test)]
    Local(shekyl_rpc_transport::local::LocalTarget),
}

/// Build the engine's verified client for `node`. A failed peer check or a
/// service at "this computer" refuses here, in core's words: the client
/// latches either, so it is never retried (Rick, 2026-11-18), and nothing
/// is written for a wallet against such a node.
pub(crate) async fn make_daemon(
    node: &EngineNode,
    network: Network,
) -> Result<DaemonClient, String> {
    let rpc: DaemonTransport = match node {
        EngineNode::Target(target) => target
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
        Err(e) if e.fault().is_target_refusal() => Err(crate::daemon_rpc::said(&e, "the node")),
        Ok(_) | Err(_) => Ok(daemon),
    }
}
