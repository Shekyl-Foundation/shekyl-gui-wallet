// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Operator coverage list and lazy shard renders (`ARCHIVAL_SHARD_SELECTION_LIST.md`).
//!
//! The GUI never fetches shard bodies. Both commands speak JSON-RPC only:
//! `get_archival_shard_coverage` (no Tor) and `request_archival_shard`
//! (`shard_id` only; the daemon draws `P` and verifies). Command names stay
//! `list_shards` / `get_shard_render` (`docs/SHARD_PREVIEW_CUTOVER.md`).

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use shekyl_shard_visual::{CandidateRecipe, ShardAggregate};
use tauri::{AppHandle, State};

use crate::daemon_rpc::{
    self, GetArchivalShardCoverageResponse, RequestArchivalShardResponse,
    ShardCoverageRow as RpcRow,
};
use crate::shard_visual::{
    cache_digest, recipe_for, render_cached, ShardRenderResponse, DEFAULT_SIZE, MAX_SIZE, MIN_SIZE,
};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ShardCoverageList {
    pub as_of_height: u64,
    pub leaf_count: u64,
    pub frozen_count: u64,
    pub settled_epoch: u64,
    pub budget_atomic: u64,
    pub sigma_work_milli: u64,
    pub profit_estimate_available: bool,
    pub shards: Vec<ShardCoverageRow>,
}

/// Tauri-wire row. Daemon JSON still carries `expected_profit_atomic` as a
/// number (`RpcRow`); JS `number` is lossy above 2^53, so this edge is a
/// decimal string and the frontend sums with `bigint`.
#[derive(Debug, Serialize)]
pub struct ShardCoverageRow {
    pub shard_id: u64,
    pub bonded_count: u64,
    pub served_count: u64,
    pub freeze_height: u64,
    pub join_scarcity_micro: u64,
    pub expected_profit_atomic: String,
}

impl From<RpcRow> for ShardCoverageRow {
    fn from(row: RpcRow) -> Self {
        Self {
            shard_id: row.shard_id,
            bonded_count: row.bonded_count,
            served_count: row.served_count,
            freeze_height: row.freeze_height,
            join_scarcity_micro: row.join_scarcity_micro,
            expected_profit_atomic: row.expected_profit_atomic.to_string(),
        }
    }
}

#[tauri::command]
pub async fn list_shards(state: State<'_, AppState>) -> Result<ShardCoverageList, String> {
    let url = state.url().await;
    let res = daemon_rpc::get_archival_shard_coverage(&state.http, &url).await?;
    Ok(coverage_list_from_rpc(res))
}

#[tauri::command]
pub async fn get_shard_render(
    app: AppHandle,
    state: State<'_, AppState>,
    shard_id: u64,
    size: Option<u32>,
) -> Result<ShardRenderResponse, String> {
    let url = state.url().await;
    let rpc = daemon_rpc::request_archival_shard(&state.http, &url, shard_id).await?;
    let aggregate = aggregate_from_rpc(shard_id, rpc)?;
    let size = size.unwrap_or(DEFAULT_SIZE).clamp(MIN_SIZE, MAX_SIZE);
    let verified_shard_id = aggregate.shard_id;
    let cache_key = cache_digest(
        &verified_shard_id.to_string(),
        aggregate.shard_hash,
        None,
        size,
    );
    // Recipe + PNG cache/render are sync I/O and 69–385 ms CPU on the
    // rule-76 floor (`shard_visual.rs` cache_digest). Keep them off the
    // async runtime so visible cards cannot stall unrelated Tauri work.
    let (recipe, png, cache_key) = tokio::task::spawn_blocking(move || {
        let recipe: CandidateRecipe = recipe_for(&aggregate, None);
        let png = render_cached(&app, &cache_key, &aggregate, None, size)?;
        Ok::<_, String>((recipe, png, cache_key))
    })
    .await
    .map_err(|e| format!("shard render task failed: {e}"))??;
    Ok(ShardRenderResponse {
        png_base64: STANDARD.encode(&png),
        recipe,
        cache_key,
        shard_id: verified_shard_id,
    })
}

fn coverage_list_from_rpc(res: GetArchivalShardCoverageResponse) -> ShardCoverageList {
    ShardCoverageList {
        as_of_height: res.as_of_height,
        leaf_count: res.leaf_count,
        frozen_count: res.frozen_count,
        settled_epoch: res.settled_epoch,
        budget_atomic: res.budget_atomic,
        sigma_work_milli: res.sigma_work_milli,
        profit_estimate_available: res.profit_estimate_available,
        shards: res.shards.into_iter().map(ShardCoverageRow::from).collect(),
    }
}

fn aggregate_from_rpc(
    requested_shard_id: u64,
    rpc: RequestArchivalShardResponse,
) -> Result<ShardAggregate, String> {
    if rpc.shard_id != requested_shard_id {
        return Err(format!(
            "daemon returned archive {} for requested {}",
            rpc.shard_id, requested_shard_id
        ));
    }
    let bytes = hex::decode(rpc.shard_hash.trim())
        .map_err(|e| format!("daemon shard_hash is not hex: {e}"))?;
    let shard_hash: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "daemon shard_hash must be 32 bytes (64 hex characters)".to_string())?;
    Ok(ShardAggregate {
        shard_id: rpc.shard_id,
        shard_hash,
        block_count: rpc.block_count,
        tx_count: rpc.tx_count,
        output_count: rpc.output_count,
        coinbase_output_count: rpc.coinbase_output_count,
        time_range_seconds: rpc.time_range_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_from_rpc_rejects_short_hash() {
        let rpc = RequestArchivalShardResponse {
            shard_id: 0,
            shard_hash: "abcd".into(),
            block_count: 1,
            tx_count: 0,
            output_count: 1,
            coinbase_output_count: 0,
            time_range_seconds: 0,
        };
        assert!(aggregate_from_rpc(0, rpc).is_err());
    }

    #[test]
    fn aggregate_from_rpc_accepts_64_hex() {
        let rpc = RequestArchivalShardResponse {
            shard_id: 7,
            shard_hash: "11".repeat(32),
            block_count: 10,
            tx_count: 2,
            output_count: 4,
            coinbase_output_count: 1,
            time_range_seconds: 120,
        };
        let agg = aggregate_from_rpc(7, rpc).expect("valid hash");
        assert_eq!(agg.shard_id, 7);
        assert_eq!(agg.block_count, 10);
        assert_eq!(agg.shard_hash[0], 0x11);
    }

    #[test]
    fn aggregate_from_rpc_rejects_mismatched_shard_id() {
        let rpc = RequestArchivalShardResponse {
            shard_id: 7,
            shard_hash: "11".repeat(32),
            block_count: 10,
            tx_count: 2,
            output_count: 4,
            coinbase_output_count: 1,
            time_range_seconds: 120,
        };
        let err = aggregate_from_rpc(1, rpc).expect_err("identity mismatch");
        assert!(
            err.contains("requested 1"),
            "mismatch must name the request: {err}"
        );
        assert!(
            err.contains("archive 7"),
            "mismatch must name the daemon id: {err}"
        );
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
