// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Operator coverage list and lazy shard views (`ARCHIVAL_SHARD_SELECTION_LIST.md`,
//! `SHARD_VIEW_FETCH.md` SV-D).
//!
//! The GUI never fetches shard bodies. `list_shards` is the daemon's
//! `shard_coverage` projection (app shell: no wallet method
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
use crate::daemon_rpc;
use crate::shard_visual::{
    cache_digest, recipe_for, render_cached, DEFAULT_SIZE, MAX_SIZE, MIN_SIZE,
};
use crate::state::AppState;
use shekyl_rpc_client::wire::archival::{Coverage, CoverageShard};
use shekyl_rpc_client::wire::ShardCoverage;
use shekyl_rpc_types::kinds::NoParams;
use shekyl_units::AtomicUnitsString;

/// The coverage list on this edge. `budget` and `work` are `null` before
/// the first settlement pass, never zero (`shard_coverage`, IG-Q61).
#[derive(Debug, Serialize)]
pub struct ShardCoverageList {
    pub as_of_height: u64,
    pub leaf_count: u64,
    pub frozen_count: u64,
    pub settled_epoch: u64,
    pub budget_atomic: Option<AtomicUnitsString>,
    pub sigma_work_milli: Option<u64>,
    pub shards: Vec<ShardCoverageRow>,
}

/// One shard on this edge. The profit is an `AtomicUnitsString` (the
/// frontend sums with `bigint`), or `null` when the node has not
/// estimated it.
#[derive(Debug, Serialize)]
pub struct ShardCoverageRow {
    pub shard_id: u64,
    pub bonded_count: u64,
    pub served_count: u64,
    pub freeze_height: u64,
    pub join_scarcity_micro: u64,
    pub expected_profit_atomic: Option<AtomicUnitsString>,
}

impl From<CoverageShard> for ShardCoverageRow {
    fn from(row: CoverageShard) -> Self {
        Self {
            shard_id: row.id.0,
            bonded_count: row.bonded_count.0,
            served_count: row.served_count.0,
            freeze_height: row.freeze_height.0,
            join_scarcity_micro: row.scarcity.0,
            expected_profit_atomic: row.profit.0,
        }
    }
}

#[tauri::command]
pub async fn list_shards(state: State<'_, AppState>) -> Result<ShardCoverageList, String> {
    let url = state.url().await;
    let res = daemon_rpc::call::<ShardCoverage>(&url, NoParams, "the shard coverage").await?;
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

fn coverage_list_from_rpc(res: Coverage) -> ShardCoverageList {
    let shards: Vec<ShardCoverageRow> = res.shard.into_iter().map(ShardCoverageRow::from).collect();
    ShardCoverageList {
        as_of_height: res.height.0,
        leaf_count: res.leaf_count.0,
        frozen_count: res.frozen_count.0,
        settled_epoch: res.settled_epoch.0,
        budget_atomic: res.budget.0,
        sigma_work_milli: res.work.0.map(|w| w.0),
        shards,
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

    /// A 4.0 `shard_coverage` reply, as the node sends it.
    fn coverage(profit: serde_json::Value, budget: serde_json::Value) -> Coverage {
        serde_json::from_value(serde_json::json!({
            "height": 1,
            "leaf_count": 0,
            "frozen_count": 1,
            "settled_epoch": 0,
            "budget": budget,
            "work": null,
            "shard": [{
                "id": 7,
                "bonded_count": 0,
                "served_count": 0,
                "freeze_height": 1,
                "scarcity": 1,
                "profit": profit,
            }],
        }))
        .expect("a 4.0 coverage reply")
    }

    #[test]
    fn coverage_list_serializes_profit_as_decimal_string() {
        let list = coverage_list_from_rpc(coverage(
            serde_json::json!("9007199254740993"),
            serde_json::json!("0"),
        ));
        let v = serde_json::to_value(&list).expect("serialize");
        let profit = &v["shards"][0]["expected_profit_atomic"];
        assert!(
            profit.is_string(),
            "JSON number would lose 2^53+1: {profit}"
        );
        assert_eq!(profit, "9007199254740993");
    }

    /// Before the first settlement pass the node estimates nothing: the
    /// list says so, and never carries a zero in its place.
    #[test]
    fn an_unestimated_coverage_is_null_not_zero() {
        let list =
            coverage_list_from_rpc(coverage(serde_json::Value::Null, serde_json::Value::Null));
        let v = serde_json::to_value(&list).expect("serialize");
        assert!(v["shards"][0]["expected_profit_atomic"].is_null());
        assert!(v["budget_atomic"].is_null());
        assert!(v["sigma_work_milli"].is_null());
    }

    /// Core's own `shard_coverage` vector maps field for field.
    #[test]
    fn core_s_coverage_vector_maps_to_the_list() {
        let list = coverage_list_from_rpc(crate::wire_vectors::reply("shard_coverage"));
        let v = serde_json::to_value(&list).expect("serialize");
        assert!(v["as_of_height"].is_u64());
        assert!(v["shards"].is_array());
        for row in v["shards"].as_array().unwrap() {
            let profit = &row["expected_profit_atomic"];
            assert!(profit.is_null() || profit.is_string(), "{profit}");
        }
    }
}
