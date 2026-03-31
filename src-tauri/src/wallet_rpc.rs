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

//! Async JSON-RPC client for shekyl-wallet-rpc.
//!
//! All methods mirror `wallet_rpc_server_commands_defs.h` in shekyl-core.
//! The wallet-rpc is expected to run in `--wallet-dir` mode so that
//! create/open/close operations are available without restart.

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
        .map_err(|e| format!("Wallet RPC connection failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Wallet RPC returned HTTP {}", resp.status()));
    }

    let rpc: JsonRpcResponse<R> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse wallet RPC response: {e}"))?;

    if let Some(err) = rpc.error {
        return Err(format!("Wallet RPC error: {}", err.message));
    }

    rpc.result
        .ok_or_else(|| "Wallet RPC returned empty result".into())
}

// ─── create_wallet ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateWalletResponse {}

pub async fn create_wallet(
    client: &Client,
    url: &str,
    filename: &str,
    password: &str,
    language: &str,
) -> Result<(), String> {
    let _resp: CreateWalletResponse = rpc_call(
        client,
        url,
        "create_wallet",
        serde_json::json!({
            "filename": filename,
            "password": password,
            "language": language,
        }),
    )
    .await?;
    Ok(())
}

// ─── open_wallet ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OpenWalletResponse {}

pub async fn open_wallet(
    client: &Client,
    url: &str,
    filename: &str,
    password: &str,
) -> Result<(), String> {
    let _resp: OpenWalletResponse = rpc_call(
        client,
        url,
        "open_wallet",
        serde_json::json!({
            "filename": filename,
            "password": password,
        }),
    )
    .await?;
    Ok(())
}

// ─── close_wallet ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CloseWalletResponse {}

pub async fn close_wallet(client: &Client, url: &str) -> Result<(), String> {
    let _resp: CloseWalletResponse =
        rpc_call(client, url, "close_wallet", serde_json::json!({})).await?;
    Ok(())
}

// ─── restore_deterministic_wallet ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RestoreWalletResponse {
    pub address: String,
    #[serde(default)]
    pub seed: String,
    #[serde(default)]
    pub info: String,
    #[serde(default)]
    pub was_deprecated: bool,
}

#[allow(clippy::too_many_arguments)]
pub async fn restore_deterministic_wallet(
    client: &Client,
    url: &str,
    filename: &str,
    seed: &str,
    password: &str,
    language: &str,
    restore_height: u64,
    seed_offset: &str,
) -> Result<RestoreWalletResponse, String> {
    rpc_call(
        client,
        url,
        "restore_deterministic_wallet",
        serde_json::json!({
            "filename": filename,
            "seed": seed,
            "password": password,
            "language": language,
            "restore_height": restore_height,
            "seed_offset": seed_offset,
        }),
    )
    .await
}

// ─── generate_from_keys ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GenerateFromKeysResponse {
    pub address: String,
    #[serde(default)]
    pub info: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_from_keys(
    client: &Client,
    url: &str,
    filename: &str,
    address: &str,
    spendkey: &str,
    viewkey: &str,
    password: &str,
    language: &str,
    restore_height: u64,
) -> Result<GenerateFromKeysResponse, String> {
    rpc_call(
        client,
        url,
        "generate_from_keys",
        serde_json::json!({
            "filename": filename,
            "address": address,
            "spendkey": spendkey,
            "viewkey": viewkey,
            "password": password,
            "language": language,
            "restore_height": restore_height,
        }),
    )
    .await
}

// ─── get_address ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GetAddressResponse {
    pub address: String,
    #[serde(default)]
    pub addresses: Vec<AddressInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AddressInfo {
    pub address: String,
    #[serde(default)]
    pub label: String,
    pub address_index: u32,
    #[serde(default)]
    pub used: bool,
}

pub async fn get_address(
    client: &Client,
    url: &str,
    account_index: u32,
) -> Result<GetAddressResponse, String> {
    rpc_call(
        client,
        url,
        "get_address",
        serde_json::json!({
            "account_index": account_index,
        }),
    )
    .await
}

// ─── get_balance ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GetBalanceResponse {
    pub balance: u64,
    pub unlocked_balance: u64,
    #[serde(default)]
    pub blocks_to_unlock: u64,
}

pub async fn get_balance(
    client: &Client,
    url: &str,
    account_index: u32,
) -> Result<GetBalanceResponse, String> {
    rpc_call(
        client,
        url,
        "get_balance",
        serde_json::json!({
            "account_index": account_index,
        }),
    )
    .await
}

// ─── query_key ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QueryKeyResponse {
    pub key: String,
}

pub async fn query_key(client: &Client, url: &str, key_type: &str) -> Result<String, String> {
    let resp: QueryKeyResponse = rpc_call(
        client,
        url,
        "query_key",
        serde_json::json!({
            "key_type": key_type,
        }),
    )
    .await?;
    Ok(resp.key)
}

// ─── get_version (health / readiness check) ──────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetVersionResponse {
    pub version: u32,
}

pub async fn get_version(client: &Client, url: &str) -> Result<u32, String> {
    let resp: GetVersionResponse =
        rpc_call(client, url, "get_version", serde_json::json!({})).await?;
    Ok(resp.version)
}

// ─── stop_wallet ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StopWalletResponse {}

pub async fn stop_wallet(client: &Client, url: &str) -> Result<(), String> {
    let _resp: StopWalletResponse =
        rpc_call(client, url, "stop_wallet", serde_json::json!({})).await?;
    Ok(())
}

// ─── get_transfers ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TransferEntry {
    #[serde(default)]
    pub txid: String,
    #[serde(default)]
    pub amount: u64,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(rename = "type", default)]
    pub transfer_type: String,
    #[serde(default)]
    pub confirmations: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GetTransfersResponse {
    #[serde(default)]
    pub r#in: Vec<TransferEntry>,
    #[serde(default)]
    pub out: Vec<TransferEntry>,
    #[serde(default)]
    pub pending: Vec<TransferEntry>,
    #[serde(default)]
    pub pool: Vec<TransferEntry>,
}

pub async fn get_transfers(
    client: &Client,
    url: &str,
    r#in: bool,
    out: bool,
    pending: bool,
    pool: bool,
) -> Result<GetTransfersResponse, String> {
    rpc_call(
        client,
        url,
        "get_transfers",
        serde_json::json!({
            "in": r#in,
            "out": out,
            "pending": pending,
            "pool": pool,
        }),
    )
    .await
}

// ─── transfer ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TransferResponse {
    #[serde(default)]
    pub tx_hash: String,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub amount: u64,
}

pub async fn transfer(
    client: &Client,
    url: &str,
    address: &str,
    amount: u64,
) -> Result<TransferResponse, String> {
    rpc_call(
        client,
        url,
        "transfer",
        serde_json::json!({
            "destinations": [{"amount": amount, "address": address}],
        }),
    )
    .await
}
