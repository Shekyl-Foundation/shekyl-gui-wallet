// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Verifying daemon client for Engine sessions (VC-4).
//!
//! Construction only: [`DaemonClient::verifying`] with
//! [`FakechainPolicy::Refuse`] (VC-R3). The session owns when that client
//! is attached; this module does not run the handshake.

use shekyl_engine_core::{DaemonClient, DaemonExpectation, FakechainPolicy, Network};
use shekyl_rpc_transport::HttpRpc;

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
    Ok(DaemonClient::verifying(
        rpc,
        DaemonExpectation {
            network,
            fakechain: FakechainPolicy::Refuse,
        },
    ))
}
