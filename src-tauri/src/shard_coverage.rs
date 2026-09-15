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

use crate::daemon_rpc::{self, GetArchivalShardCoverageResponse, RequestArchivalShardResponse};
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
    pub shards: Vec<daemon_rpc::ShardCoverageRow>,
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
    let aggregate = aggregate_from_rpc(rpc)?;
    let size = size.unwrap_or(DEFAULT_SIZE).clamp(MIN_SIZE, MAX_SIZE);
    let cache_key = cache_digest(
        &aggregate.shard_id.to_string(),
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
        shard_id,
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
        shards: res.shards,
    }
}

fn aggregate_from_rpc(rpc: RequestArchivalShardResponse) -> Result<ShardAggregate, String> {
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
        assert!(aggregate_from_rpc(rpc).is_err());
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
        let agg = aggregate_from_rpc(rpc).expect("valid hash");
        assert_eq!(agg.shard_id, 7);
        assert_eq!(agg.block_count, 10);
        assert_eq!(agg.shard_hash[0], 0x11);
    }
}
