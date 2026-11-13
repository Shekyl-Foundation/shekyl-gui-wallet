// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The daemon this wallet talks to: where it points, what pointing there
//! discloses, and how the node is doing.
//!
//! Carved out of `commands.rs` when the disclosure surface pushed that file
//! back over its ratchet ceiling. The three belong together: they are the
//! same question asked three ways, and every one of them reads
//! `AppState::daemon_url`. Splitting by feature is what the ratchet asks
//! for instead of a raised ceiling
//! (`.cursor/rules/27-composition-decomposition.mdc`).

use serde::Serialize;
use tauri::State;

use crate::daemon_rpc;
use crate::state::{AppState, NetworkType};
use shekyl_rpc_client::wire::block::{BlockHeader, BlockSelect, HeaderSelect};
use shekyl_rpc_client::wire::chain::{Curve, CurveSelect};
use shekyl_rpc_client::wire::daemon::DaemonStatus as Status;
use shekyl_rpc_client::wire::shared::Selector;
use shekyl_rpc_client::wire::{BlockHeaderShow, CurveShow, DaemonStatus};
use shekyl_rpc_types::kinds::NoParams;
use shekyl_units::AtomicUnitsString;

/// What setting a daemon URL exposes, returned to the panel that set it.
///
/// `warnings` is empty for a daemon on this machine, which is the default
/// and the supported configuration. A daemon that is not loopback carries
/// the §1 operator statement of shekyl-core's `RPC_TRANSPORT_POSTURE.md`:
/// whoever runs that daemon sees which blocks this wallet asks for and what
/// it broadcasts. The wallet does not refuse it — the operator may well own
/// the far end — it says what is true, where the address was typed
/// (RT-W7: discouragement where it is consumed).
#[derive(Debug, Clone, Serialize)]
pub struct DaemonConnection {
    /// The URL now in effect.
    pub url: String,
    /// Disclosures for that URL; never an assurance, only ever warnings.
    pub warnings: Vec<String>,
}

/// The disclosures for a daemon URL the wallet is about to use.
///
/// This wallet dials the daemon directly and has no proxy, so the
/// network-path half of the disclosure is judged with no proxy configured
/// and `AlwaysRemote` is the honest resolution: there is no proxy for a
/// hostname to leak *past*, and claiming a local-resolver leak would assert
/// a lookup no configured transport performs.
fn daemon_disclosures(url: &str) -> Vec<String> {
    shekyl_rpc_transport::network_posture::daemon_disclosures(
        "daemon URL",
        url,
        None,
        shekyl_rpc_transport::network_posture::ProxyResolution::AlwaysRemote,
    )
    .into_iter()
    .collect()
}

#[tauri::command]
pub async fn set_daemon_connection(
    state: State<'_, AppState>,
    network: String,
    url: Option<String>,
) -> Result<DaemonConnection, String> {
    let net: NetworkType = serde_json::from_value(serde_json::Value::String(network))
        .map_err(|_| "Invalid network: must be mainnet, testnet, or stagenet")?;

    let new_url =
        url.unwrap_or_else(|| format!("http://127.0.0.1:{}/json_rpc", net.default_rpc_port()));

    let warnings = daemon_disclosures(&new_url);
    *state.daemon_url.write().await = new_url.clone();
    *state.network.write().await = net;

    Ok(DaemonConnection {
        url: new_url,
        warnings,
    })
}

/// The disclosures for the daemon URL already in effect, so a wallet that
/// starts with a non-loopback daemon says so too — not only the session in
/// which it was typed.
#[tauri::command]
pub async fn daemon_connection_disclosures(
    state: State<'_, AppState>,
) -> Result<DaemonConnection, String> {
    let url = state.url().await;
    let warnings = daemon_disclosures(&url);
    Ok(DaemonConnection { url, warnings })
}

/// The chain as the node reports it (`daemon_status`), with the tip's
/// header (`block_header_show`) and curve tree (`curve_show`). Field names
/// follow the 4.0 wire (shekyl-core `INPUT_GRAMMAR.md`, the `daemon_status`
/// table). A read the node did not answer is `null`, never zero.
#[derive(Debug, Serialize)]
pub struct ChainHealth {
    /// The wire's `height` unchanged: the tip's own block number (genesis
    /// is 0), shown as "Block number".
    pub height: u64,
    pub hash: String,
    /// A 128-bit value, as a decimal string.
    pub difficulty: String,
    pub tx_count: u64,
    /// `null` while the node's pool is retired.
    pub txpool_count: Option<u64>,
    pub synchronized: bool,
    /// Coins emitted so far.
    pub emission: AtomicUnitsString,
    /// The emission multiplier, in millionths.
    pub emission_multiplier: u64,
    /// The burn rate, in millionths.
    pub burn: u64,
    pub burn_total: AtomicUnitsString,
    /// The stakers' effective share of emission, in millionths.
    pub stake_share: u64,
    /// The chain's block spacing, seconds.
    pub block_spacing: u64,
    /// Blocks a mined reward waits before it can be spent: core's
    /// consensus constant (`shekyl_consensus::COINBASE_LOCK_WINDOW`), not a
    /// figure the node reports, carried here beside the spacing so a page
    /// never holds its own copy.
    pub coinbase_lock_window: u64,
    pub network: String,
    pub last_block: Option<LastBlock>,
    pub curve_tree: Option<CurveTree>,
}

/// The tip's header.
#[derive(Debug, Serialize)]
pub struct LastBlock {
    pub reward: AtomicUnitsString,
    /// Unix seconds.
    pub time: u64,
    pub hash: String,
    /// Bytes.
    pub weight: u64,
}

/// The curve tree at the tip.
#[derive(Debug, Serialize)]
pub struct CurveTree {
    pub root: String,
    pub layers: u64,
    pub leaf_count: u64,
}

#[tauri::command]
pub async fn get_chain_health(state: State<'_, AppState>) -> Result<ChainHealth, String> {
    let url = state.url().await;
    let network = *state.network.read().await;

    let status = daemon_rpc::call::<DaemonStatus>(&url, NoParams, "the node's status").await?;
    let tip = HeaderSelect(BlockSelect {
        select: Selector::Tip,
        pow: false,
        prune: None,
    });
    let header = daemon_rpc::call::<BlockHeaderShow>(&url, tip, "the last block")
        .await
        .ok();
    let curve = daemon_rpc::call::<CurveShow>(&url, CurveSelect(Selector::Tip), "the curve tree")
        .await
        .ok();
    Ok(chain_health(status, header, curve, network.as_str()))
}

/// The chain read from the node's three replies. The header and the curve
/// are `None` when the node did not answer them, and stay `null` on the
/// page.
fn chain_health(
    status: Status,
    header: Option<BlockHeader>,
    curve: Option<Curve>,
    network: &str,
) -> ChainHealth {
    ChainHealth {
        height: status.height.0,
        hash: status.hash.to_string(),
        difficulty: status.difficulty.get().to_string(),
        tx_count: status.tx_count.0,
        txpool_count: status.txpool_count.0.map(|c| c.0),
        synchronized: status.synchronized,
        emission: status.emission,
        emission_multiplier: status.emission_multiplier.0,
        burn: status.burn.0,
        burn_total: status.burn_total,
        stake_share: status.stake_share.0,
        block_spacing: status.block_spacing.0,
        coinbase_lock_window: u64::try_from(shekyl_consensus::COINBASE_LOCK_WINDOW)
            .expect("the coinbase lock window fits u64"),
        network: network.into(),
        last_block: header.map(|h| LastBlock {
            reward: h.reward,
            time: h.time.0,
            hash: h.hash.to_string(),
            weight: h.weight.0,
        }),
        curve_tree: curve.map(|c| CurveTree {
            root: c.root.to_string(),
            layers: c.layers.0,
            leaf_count: c.leaf_count.0,
        }),
    }
}

#[cfg(test)]
mod chain_health_tests {
    use super::*;
    use crate::wire_vectors::reply;

    /// Core's own 4.0 replies become the page's chain read, each figure
    /// from its 4.0 field (`INPUT_GRAMMAR.md`'s `daemon_status` table).
    #[test]
    fn the_chain_read_is_built_from_core_s_4_0_replies() {
        let health = chain_health(
            reply("daemon_status"),
            Some(reply("block_header_show")),
            Some(reply("curve_show")),
            "testnet",
        );
        let v = serde_json::to_value(&health).unwrap();
        // The tip's own number, unchanged: tip 1199 shows 1,199.
        assert_eq!(v["height"], 1199);
        assert_eq!(
            v["coinbase_lock_window"],
            shekyl_consensus::COINBASE_LOCK_WINDOW as u64
        );
        assert!(v.get("target_height").is_none(), "nothing reads it");
        assert_eq!(
            v["difficulty"],
            (0x2a3f1c_u64).to_string(),
            "the hex128, as decimal"
        );
        assert_eq!(v["emission"], "50212500000000");
        assert_eq!(v["emission_multiplier"], 1_000_000);
        assert_eq!(v["burn"], 50_000);
        assert_eq!(v["burn_total"], "1250000000");
        assert_eq!(v["stake_share"], 300_000);
        assert_eq!(v["txpool_count"], 2);
        assert_eq!(v["block_spacing"], 120);
        assert!(v["last_block"]["reward"].is_string());
        assert!(v["curve_tree"]["layers"].is_u64());
        assert_eq!(v["network"], "testnet");
    }

    /// A header or curve the node did not answer is `null`, never zeros.
    #[test]
    fn an_unanswered_header_or_curve_is_null() {
        let v = serde_json::to_value(chain_health(reply("daemon_status"), None, None, "mainnet"))
            .unwrap();
        assert!(v["last_block"].is_null());
        assert!(v["curve_tree"].is_null());
    }
}
