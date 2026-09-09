// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Verifying daemon client for Engine sessions (VC-4).
//!
//! Construction plus the identity handshake: [`DaemonClient::verifying`]
//! with [`FakechainPolicy::Refuse`] (VC-R3), then one Engine RPC so a
//! mismatch is known **before** `Engine::create` writes a wallet file.
//! Transport failures are not stored and are not fatal — an unreachable
//! daemon still lets create/open proceed (offline). The session owns when
//! the client is attached.

use shekyl_engine_core::{DaemonClient, DaemonExpectation, FakechainPolicy, Network};
use shekyl_rpc_client::Rpc;
use shekyl_rpc_transport::HttpRpc;

use crate::engine_errors::identity_refusal_message;

pub(crate) async fn make_daemon(
    daemon_http_base: &str,
    network: Network,
) -> Result<DaemonClient, String> {
    let trimmed = daemon_http_base.trim_end_matches('/');
    let url = trimmed
        .strip_suffix("/json_rpc")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_owned();
    let rpc = HttpRpc::new(url)
        .await
        .map_err(|e| format!("daemon unreachable: {e}"))?;
    let daemon = DaemonClient::verifying(
        rpc,
        DaemonExpectation {
            network,
            fakechain: FakechainPolicy::Refuse,
        },
    );
    // First Engine RPC runs the four-axis handshake. Identity mismatch
    // refuses here so create/restore never write a file the person cannot
    // recover (`get_seed` is create-once). Unreachable is not a mismatch.
    match daemon.get_height().await {
        Ok(_) => Ok(daemon),
        Err(e) => {
            let rendered = e.to_string();
            if let Some(msg) = identity_refusal_message(&rendered) {
                Err(msg)
            } else {
                Ok(daemon)
            }
        }
    }
}
