// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! Async JSON-RPC client for the shekyld daemon.
//!
//! All responses mirror the field names from `core_rpc_server_commands_defs.h`.
//! Fixed-point u64 economic values use SCALE = 1_000_000.

use reqwest::Client;
use serde::{Deserialize, Serialize};

// ─── JSON-RPC envelope ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonRpcRequest<P: Serialize> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'static str,
    params: P,
}

#[derive(Deserialize)]
struct JsonRpcResponse<R> {
    result: Option<R>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    message: String,
}

async fn rpc_call<P: Serialize, R: for<'de> Deserialize<'de>>(
    client: &Client,
    url: &str,
    method: &'static str,
    params: P,
) -> Result<R, String> {
    let body = JsonRpcRequest {
        jsonrpc: "2.0",
        id: "0",
        method,
        params,
    };

    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Daemon connection failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Daemon returned HTTP {}", resp.status()));
    }

    let rpc: JsonRpcResponse<R> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse daemon response: {e}"))?;

    if let Some(err) = rpc.error {
        return Err(format!("Daemon RPC error: {}", err.message));
    }

    rpc.result
        .ok_or_else(|| "Daemon returned empty result".into())
}

// ─── get_info ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetInfoResponse {
    pub height: u64,
    pub target_height: u64,
    pub top_block_hash: String,
    pub difficulty: u64,
    pub tx_count: u64,
    pub tx_pool_size: u64,
    pub database_size: u64,
    pub version: String,
    #[serde(default)]
    pub synchronized: bool,
    pub already_generated_coins: Option<String>,
    #[serde(default)]
    pub release_multiplier: u64,
    #[serde(default)]
    pub burn_pct: u64,
    #[serde(default)]
    pub stake_ratio: u64,
    #[serde(default)]
    pub total_burned: u64,
    #[serde(default)]
    pub staker_pool_balance: u64,
    #[serde(default)]
    pub staker_emission_share_effective: u64,
    #[serde(default)]
    pub emission_era: String,
}

pub async fn get_info(client: &Client, url: &str) -> Result<GetInfoResponse, String> {
    rpc_call(client, url, "get_info", serde_json::json!({})).await
}

// ─── get_last_block_header ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BlockHeader {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub reward: u64,
    pub difficulty: u64,
    pub block_size: u64,
}

#[derive(Deserialize)]
struct BlockHeaderWrapper {
    block_header: BlockHeader,
}

pub async fn get_last_block_header(client: &Client, url: &str) -> Result<BlockHeader, String> {
    let wrapper: BlockHeaderWrapper =
        rpc_call(client, url, "get_last_block_header", serde_json::json!({})).await?;
    Ok(wrapper.block_header)
}

// ─── get_staking_info ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GetStakingInfoResponse {
    pub height: u64,
    #[serde(default)]
    pub stake_ratio: u64,
    #[serde(default)]
    pub total_staked: u64,
    #[serde(default)]
    pub staker_pool_balance: u64,
    #[serde(default)]
    pub staker_emission_share: u64,
    #[serde(default)]
    pub tier_0_lock_blocks: u64,
    #[serde(default)]
    pub tier_1_lock_blocks: u64,
    #[serde(default)]
    pub tier_2_lock_blocks: u64,
}

pub async fn get_staking_info(
    client: &Client,
    url: &str,
) -> Result<GetStakingInfoResponse, String> {
    rpc_call(client, url, "get_staking_info", serde_json::json!({})).await
}

// ─── estimate_claim_reward ───────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EstimateClaimRewardResponse {
    pub reward: u64,
    pub tier: u8,
    pub staked_amount: u64,
}

// ─── Plain HTTP JSON helpers (non-JSON-RPC endpoints) ────────────────────────

async fn http_json_call<P: Serialize, R: for<'de> Deserialize<'de>>(
    client: &Client,
    base_url: &str,
    path: &str,
    params: P,
) -> Result<R, String> {
    let url = format!("{}{}", base_url, path);
    let resp = client
        .post(&url)
        .json(&params)
        .send()
        .await
        .map_err(|e| format!("Daemon connection failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Daemon returned HTTP {}", resp.status()));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse daemon response: {e}"))
}

// ─── mining_status ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MiningStatusResponse {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub speed: u64,
    #[serde(default)]
    pub threads_count: u32,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub pow_algorithm: String,
    #[serde(default)]
    pub is_background_mining_enabled: bool,
    #[serde(default)]
    pub block_target: u32,
    #[serde(default)]
    pub block_reward: u64,
    #[serde(default)]
    pub difficulty: u64,
}

pub async fn mining_status(
    client: &Client,
    base_url: &str,
) -> Result<MiningStatusResponse, String> {
    http_json_call(client, base_url, "/mining_status", serde_json::json!({})).await
}

// ─── start_mining ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DaemonStatusResponse {
    status: String,
}

pub async fn start_mining(
    client: &Client,
    base_url: &str,
    miner_address: &str,
    threads_count: u32,
    do_background_mining: bool,
    ignore_battery: bool,
) -> Result<(), String> {
    let resp: DaemonStatusResponse = http_json_call(
        client,
        base_url,
        "/start_mining",
        serde_json::json!({
            "miner_address": miner_address,
            "threads_count": threads_count,
            "do_background_mining": do_background_mining,
            "ignore_battery": ignore_battery,
        }),
    )
    .await?;

    if resp.status == "OK" {
        Ok(())
    } else {
        Err(format!("Daemon rejected start_mining: {}", resp.status))
    }
}

// ─── stop_mining ─────────────────────────────────────────────────────────────

pub async fn stop_mining(client: &Client, base_url: &str) -> Result<(), String> {
    let resp: DaemonStatusResponse =
        http_json_call(client, base_url, "/stop_mining", serde_json::json!({})).await?;

    if resp.status == "OK" {
        Ok(())
    } else {
        Err(format!("Daemon rejected stop_mining: {}", resp.status))
    }
}

// ─── estimate_claim_reward ───────────────────────────────────────────────────

#[allow(dead_code)]
pub async fn estimate_claim_reward(
    client: &Client,
    url: &str,
    staked_output_index: u64,
    from_height: u64,
    to_height: u64,
) -> Result<EstimateClaimRewardResponse, String> {
    rpc_call(
        client,
        url,
        "estimate_claim_reward",
        serde_json::json!({
            "staked_output_index": staked_output_index,
            "from_height": from_height,
            "to_height": to_height,
        }),
    )
    .await
}
