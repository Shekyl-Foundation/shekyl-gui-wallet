// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Operator coverage list and lazy shard views (`ARCHIVAL_SHARD_SELECTION_LIST.md`,
//! `SHARD_VIEW_FETCH.md` SV-D).
//!
//! The GUI never fetches shard bodies. `list_shards` is the daemon's
//! `get_archival_shard_coverage` projection (app shell: no wallet method
//! lists coverage). `get_shard_view` is the contract adapter: the open
//! wallet's daemon fetches the shard from a holder and answers the
//! aggregate (`shekyl_wallet_contract::shard_view::fetch_shard_view`, the
//! same call `shekyl-wallet-rpc` serves), and this edge draws candidate.v1
//! from it because a page cannot. The daemon's refusals arrive as the
//! contract's codes — `SHARD_STILL_OPEN`, `SHARD_UNAVAILABLE`,
//! `SHARD_VIEW_NOT_OFFERED` — so a card shows a state, never an empty frame.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use shekyl_shard_visual::{CandidateRecipe, ShardAggregate};
use shekyl_wallet_contract::error::WalletRpcError;
use shekyl_wallet_contract::shard_view::{fetch_shard_view, ShardViewResult};
use tauri::{AppHandle, State};

use crate::contract_error::{open_engine, ContractError};
use crate::daemon_rpc::{self, GetArchivalShardCoverageResponse, ShardCoverageRow as RpcRow};
use crate::shard_visual::{
    cache_digest, recipe_for, render_cached, DEFAULT_SIZE, MAX_SIZE, MIN_SIZE,
};
use crate::state::AppState;
use shekyl_units::{AtomicUnits, AtomicUnitsString};

#[derive(Debug, Serialize)]
pub struct ShardCoverageList {
    pub as_of_height: u64,
    pub leaf_count: u64,
    pub frozen_count: u64,
    pub settled_epoch: u64,
    pub budget_atomic: AtomicUnitsString,
    pub sigma_work_milli: u64,
    pub profit_estimate_available: bool,
    pub shards: Vec<ShardCoverageRow>,
}

/// Tauri-wire row. Daemon JSON still carries `expected_profit_atomic` as a
/// number (`RpcRow`); on this edge it is an `AtomicUnitsString` (see `wire`)
/// and the frontend sums with `bigint`.
#[derive(Debug, Serialize)]
pub struct ShardCoverageRow {
    pub shard_id: u64,
    pub bonded_count: u64,
    pub served_count: u64,
    pub freeze_height: u64,
    pub join_scarcity_micro: u64,
    pub expected_profit_atomic: AtomicUnitsString,
}

impl From<RpcRow> for ShardCoverageRow {
    fn from(row: RpcRow) -> Self {
        Self {
            shard_id: row.shard_id,
            bonded_count: row.bonded_count,
            served_count: row.served_count,
            freeze_height: row.freeze_height,
            join_scarcity_micro: row.join_scarcity_micro,
            expected_profit_atomic: AtomicUnits::from_raw(row.expected_profit_atomic).into(),
        }
    }
}

#[tauri::command]
pub async fn list_shards(state: State<'_, AppState>) -> Result<ShardCoverageList, String> {
    let url = state.url().await;
    let res = daemon_rpc::get_archival_shard_coverage(&state.http, &url).await?;
    Ok(coverage_list_from_rpc(res))
}

/// `get_shard_view`'s answer on this edge: the contract's result plus the
/// candidate.v1 render a page cannot draw itself. One DTO (rule 27); the
/// picture is derived from `view`, never carried separately on any wire.
#[derive(Debug, Serialize)]
pub struct ShardViewRender {
    pub view: ShardViewResult,
    pub png_base64: String,
    pub recipe: CandidateRecipe,
    pub cache_key: String,
}

#[tauri::command]
pub async fn get_shard_view(
    app: AppHandle,
    state: State<'_, AppState>,
    shard_id: u64,
    size: Option<u32>,
) -> Result<ShardViewRender, ContractError> {
    let shared = open_engine(&state).await?;
    // Clone the handle and drop the read guard: the daemon fetches the body
    // from a holder before it answers, and no other command should wait on
    // the engine for that.
    let daemon = shared.read().await.daemon().clone();
    let view = fetch_shard_view(&daemon, shard_id)
        .await
        .map_err(ContractError::from_rpc)?;
    if view.shard_id != shard_id {
        // The contract's own answer names another shard: the wallet broke
        // its contract, which is the protocol-violation code, not a bad shard.
        return Err(ContractError::from_rpc(
            WalletRpcError::DaemonProtocolViolation,
        ));
    }
    let aggregate = aggregate_from_view(&view)?;
    let size = size.unwrap_or(DEFAULT_SIZE).clamp(MIN_SIZE, MAX_SIZE);
    let cache_key = cache_digest(&view.shard_id.to_string(), aggregate.shard_hash, None, size);
    // Recipe + PNG cache/render are sync I/O and 69–385 ms CPU on the
    // rule-76 floor (`shard_visual.rs` cache_digest). Keep them off the
    // async runtime so visible cards cannot stall unrelated Tauri work.
    let (recipe, png, cache_key) = tokio::task::spawn_blocking(move || {
        let recipe: CandidateRecipe = recipe_for(&aggregate, None);
        let png = render_cached(&app, &cache_key, &aggregate, None, size)?;
        Ok::<_, String>((recipe, png, cache_key))
    })
    .await
    .map_err(|e| format!("shard render task failed: {e}"))
    .and_then(|r| r)
    .map_err(|detail| ContractError::from_rpc(WalletRpcError::InternalError(detail)))?;
    Ok(ShardViewRender {
        view,
        png_base64: STANDARD.encode(&png),
        recipe,
        cache_key,
    })
}

fn coverage_list_from_rpc(res: GetArchivalShardCoverageResponse) -> ShardCoverageList {
    ShardCoverageList {
        as_of_height: res.as_of_height,
        leaf_count: res.leaf_count,
        frozen_count: res.frozen_count,
        settled_epoch: res.settled_epoch,
        budget_atomic: AtomicUnits::from_raw(res.budget_atomic).into(),
        sigma_work_milli: res.sigma_work_milli,
        profit_estimate_available: res.profit_estimate_available,
        shards: res.shards.into_iter().map(ShardCoverageRow::from).collect(),
    }
}

/// The renderer's input from the contract's result. The result's hash is
/// the contract's `^[0-9a-f]{64}$`; one that is not is the wallet breaking
/// its contract, reported as such rather than drawn under a guessed hash.
fn aggregate_from_view(view: &ShardViewResult) -> Result<ShardAggregate, ContractError> {
    let violation = || ContractError::from_rpc(WalletRpcError::DaemonProtocolViolation);
    let bytes = hex::decode(&view.shard_hash).map_err(|_| violation())?;
    let shard_hash: [u8; 32] = bytes.try_into().map_err(|_| violation())?;
    Ok(ShardAggregate {
        shard_id: view.shard_id,
        shard_hash,
        block_count: view.block_count,
        tx_count: view.tx_count,
        output_count: view.output_count,
        coinbase_output_count: view.coinbase_output_count,
        time_range_seconds: view.time_range_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(shard_hash: &str) -> ShardViewResult {
        ShardViewResult {
            shard_id: 7,
            shard_hash: shard_hash.to_owned(),
            archival_len: 4_000_000,
            block_count: 10,
            tx_count: 2,
            output_count: 4,
            coinbase_output_count: 1,
            time_range_seconds: 120,
            close_height: 9_000,
        }
    }

    #[test]
    fn aggregate_from_view_takes_the_contracts_hash() {
        let agg = aggregate_from_view(&view(&"11".repeat(32))).expect("valid hash");
        assert_eq!(agg.shard_id, 7);
        assert_eq!(agg.block_count, 10);
        assert_eq!(agg.shard_hash[0], 0x11);
    }

    #[test]
    fn a_hash_off_the_contract_is_a_protocol_violation_not_a_render() {
        for bad in ["abcd", &"zz".repeat(32), &"11".repeat(33)] {
            let err = aggregate_from_view(&view(bad)).expect_err("off-contract hash");
            assert_eq!(err.code, "DAEMON_PROTOCOL_VIOLATION", "{bad}: {err:?}");
        }
    }

    #[test]
    fn coverage_list_serializes_profit_as_decimal_string() {
        let rpc = GetArchivalShardCoverageResponse {
            as_of_height: 1,
            leaf_count: 0,
            frozen_count: 1,
            settled_epoch: 0,
            budget_atomic: 0,
            sigma_work_milli: 0,
            profit_estimate_available: true,
            shards: vec![RpcRow {
                shard_id: 7,
                bonded_count: 0,
                served_count: 0,
                freeze_height: 1,
                join_scarcity_micro: 1,
                expected_profit_atomic: (1u64 << 53) + 1,
            }],
        };
        let list = coverage_list_from_rpc(rpc);
        let v = serde_json::to_value(&list).expect("serialize");
        let profit = &v["shards"][0]["expected_profit_atomic"];
        assert!(
            profit.is_string(),
            "JSON number would lose 2^53+1: {profit}"
        );
        assert_eq!(profit, "9007199254740993");
    }
}
