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

#[derive(Debug, Serialize)]
pub struct ChainHealth {
    pub height: u64,
    pub target_height: u64,
    pub top_block_hash: String,
    pub difficulty: u64,
    pub tx_count: u64,
    pub tx_pool_size: u64,
    pub database_size: u64,
    pub version: String,
    pub synchronized: bool,
    pub already_generated_coins: String,
    pub release_multiplier: u64,
    pub burn_pct: u64,
    pub stake_ratio: u64,
    pub total_burned: u64,
    pub staker_pool_balance: u64,
    pub staker_emission_share_effective: u64,
    pub emission_era: String,
    pub last_block_reward: u64,
    pub last_block_timestamp: u64,
    pub last_block_hash: String,
    pub last_block_size: u64,
    pub total_staked: u64,
    pub tier_0_lock_blocks: u64,
    pub tier_1_lock_blocks: u64,
    pub tier_2_lock_blocks: u64,
    pub network: String,
    pub curve_tree_root: String,
    pub curve_tree_leaf_count: u64,
    pub curve_tree_depth: u8,
}

#[tauri::command]
pub async fn get_chain_health(state: State<'_, AppState>) -> Result<ChainHealth, String> {
    let url = state.url().await;
    let network = state.network.read().await;

    let info = daemon_rpc::get_info(&state.http, &url).await?;
    let block = daemon_rpc::get_last_block_header(&state.http, &url)
        .await
        .ok();
    let staking = daemon_rpc::get_staking_info(&state.http, &url).await.ok();
    let tree = daemon_rpc::get_curve_tree_info(&state.http, &url)
        .await
        .ok();

    Ok(ChainHealth {
        height: info.height,
        target_height: info.target_height,
        top_block_hash: info.top_block_hash,
        difficulty: info.difficulty,
        tx_count: info.tx_count,
        tx_pool_size: info.tx_pool_size,
        database_size: info.database_size,
        version: info.version,
        synchronized: info.synchronized,
        already_generated_coins: info.already_generated_coins.unwrap_or_default(),
        release_multiplier: info.release_multiplier,
        burn_pct: info.burn_pct,
        stake_ratio: info.stake_ratio,
        total_burned: info.total_burned,
        staker_pool_balance: info.staker_pool_balance,
        staker_emission_share_effective: info.staker_emission_share_effective,
        emission_era: info.emission_era,
        last_block_reward: block.as_ref().map_or(0, |b| b.reward),
        last_block_timestamp: block.as_ref().map_or(0, |b| b.timestamp),
        last_block_hash: block.as_ref().map_or_else(String::new, |b| b.hash.clone()),
        last_block_size: block.as_ref().map_or(0, |b| b.block_size),
        total_staked: staking.as_ref().map_or(0, |s| s.total_staked),
        tier_0_lock_blocks: staking.as_ref().map_or(0, |s| s.tier_0_lock_blocks),
        tier_1_lock_blocks: staking.as_ref().map_or(0, |s| s.tier_1_lock_blocks),
        tier_2_lock_blocks: staking.as_ref().map_or(0, |s| s.tier_2_lock_blocks),
        network: network.as_str().into(),
        curve_tree_root: tree.as_ref().map_or_else(String::new, |t| t.root.clone()),
        curve_tree_leaf_count: tree.as_ref().map_or(0, |t| t.leaf_count),
        curve_tree_depth: tree.as_ref().map_or(0, |t| t.depth),
    })
}
