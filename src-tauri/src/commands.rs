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

//! Tauri commands for the Shekyl wallet.
//!
//! Chain/staking/mining commands call the daemon via JSON-RPC.
//! Wallet commands call shekyl-wallet-rpc via JSON-RPC.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::daemon_rpc;
use crate::state::{AppState, NetworkType};
use crate::wallet_process;
use crate::wallet_rpc;

const SCALE: f64 = 1_000_000.0;
const BLOCKS_PER_YEAR: f64 = 262_800.0; // 2-minute blocks

// ─── Data types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WalletStatus {
    pub connected: bool,
    pub wallet_open: bool,
    pub wallet_name: Option<String>,
    pub daemon_address: Option<String>,
    pub network: String,
    pub synced: bool,
    pub sync_height: u64,
    pub daemon_height: u64,
}

#[derive(Debug, Serialize)]
pub struct WalletInfo {
    pub name: String,
    pub address: String,
    pub seed_language: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct CreateWalletResult {
    pub name: String,
    pub address: String,
    pub seed: String,
    pub seed_language: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct WalletFileInfo {
    pub name: String,
    pub path: String,
    pub modified: u64,
}

#[derive(Debug, Serialize)]
pub struct Balance {
    pub total: u64,
    pub unlocked: u64,
    pub staked: u64,
}

#[derive(Debug, Serialize)]
pub struct TxInfo {
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    pub height: u64,
    pub timestamp: u64,
    pub direction: String,
    pub confirmed: bool,
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
}

#[derive(Debug, Serialize)]
pub struct TierYield {
    pub tier: u8,
    pub lock_blocks: u64,
    pub lock_duration_hours: f64,
    pub yield_multiplier: f64,
    pub estimated_apy: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PqcStatus {
    pub enabled: bool,
    pub scheme: String,
    pub classical: String,
    pub post_quantum: String,
    pub tx_version: u8,
    pub description: String,
}

// ─── Daemon-connected commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn get_wallet_status(state: State<'_, AppState>) -> Result<WalletStatus, String> {
    let url = state.url().await;
    let network = state.network.read().await;
    let wallet_open = *state.wallet_open.read().await;
    let wallet_name = state.wallet_name.read().await.clone();

    match daemon_rpc::get_info(&state.http, &url).await {
        Ok(info) => Ok(WalletStatus {
            connected: true,
            wallet_open,
            wallet_name,
            daemon_address: Some(url),
            network: network.as_str().into(),
            synced: info.synchronized,
            sync_height: info.height,
            daemon_height: info.target_height,
        }),
        Err(_) => Ok(WalletStatus {
            connected: false,
            wallet_open,
            wallet_name,
            daemon_address: Some(url),
            network: network.as_str().into(),
            synced: false,
            sync_height: 0,
            daemon_height: 0,
        }),
    }
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
    })
}

#[tauri::command]
pub async fn get_tier_yields(state: State<'_, AppState>) -> Result<Vec<TierYield>, String> {
    let url = state.url().await;
    let staking = daemon_rpc::get_staking_info(&state.http, &url).await?;
    let info = daemon_rpc::get_info(&state.http, &url).await?;

    let emission_share = info.staker_emission_share_effective as f64 / SCALE;

    let tiers = [
        (0u8, staking.tier_0_lock_blocks, 1.0),
        (1, staking.tier_1_lock_blocks, 1.5),
        (2, staking.tier_2_lock_blocks, 2.0),
    ];

    let total_staked = staking.total_staked.max(1) as f64;

    Ok(tiers
        .iter()
        .map(|&(tier, lock_blocks, multiplier)| {
            let lock_hours = lock_blocks as f64 * 2.0 / 60.0;
            let locks_per_year = BLOCKS_PER_YEAR / lock_blocks.max(1) as f64;
            let estimated_apy =
                emission_share * multiplier * locks_per_year / (total_staked / 1e9) * 100.0;

            TierYield {
                tier,
                lock_blocks,
                lock_duration_hours: lock_hours,
                yield_multiplier: multiplier,
                estimated_apy: estimated_apy.min(999.9),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn set_daemon_connection(
    state: State<'_, AppState>,
    network: String,
    url: Option<String>,
) -> Result<bool, String> {
    let net: NetworkType = serde_json::from_value(serde_json::Value::String(network))
        .map_err(|_| "Invalid network: must be mainnet, testnet, or stagenet")?;

    let new_url =
        url.unwrap_or_else(|| format!("http://127.0.0.1:{}/json_rpc", net.default_rpc_port()));

    *state.daemon_url.write().await = new_url;
    *state.network.write().await = net;

    let new_wallet_url = format!(
        "http://127.0.0.1:{}/json_rpc",
        net.default_wallet_rpc_port()
    );
    *state.wallet_rpc_url.write().await = new_wallet_url;

    Ok(true)
}

#[tauri::command]
pub async fn get_pqc_status() -> Result<PqcStatus, String> {
    Ok(PqcStatus {
        enabled: true,
        scheme: "Hybrid".into(),
        classical: "Ed25519".into(),
        post_quantum: "ML-DSA-65 (FIPS 204)".into(),
        tx_version: 3,
        description: "All spends protected by hybrid Ed25519 + ML-DSA-65 signatures".into(),
    })
}

// ─── Mining commands ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MiningStatus {
    pub active: bool,
    pub speed: u64,
    pub threads_count: u32,
    pub address: String,
    pub pow_algorithm: String,
    pub is_background_mining_enabled: bool,
    pub block_target: u32,
    pub block_reward: u64,
    pub difficulty: u64,
}

#[tauri::command]
pub async fn get_mining_status(state: State<'_, AppState>) -> Result<MiningStatus, String> {
    let base = state.base_url().await;
    let ms = daemon_rpc::mining_status(&state.http, &base).await?;
    Ok(MiningStatus {
        active: ms.active,
        speed: ms.speed,
        threads_count: ms.threads_count,
        address: ms.address,
        pow_algorithm: ms.pow_algorithm,
        is_background_mining_enabled: ms.is_background_mining_enabled,
        block_target: ms.block_target,
        block_reward: ms.block_reward,
        difficulty: ms.difficulty,
    })
}

#[tauri::command]
pub async fn start_mining_cmd(
    state: State<'_, AppState>,
    address: String,
    threads: u32,
    background: bool,
) -> Result<bool, String> {
    let base = state.base_url().await;
    daemon_rpc::start_mining(&state.http, &base, &address, threads, background, true).await?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_mining_cmd(state: State<'_, AppState>) -> Result<bool, String> {
    let base = state.base_url().await;
    daemon_rpc::stop_mining(&state.http, &base).await?;
    Ok(true)
}

// ─── Wallet startup commands ─────────────────────────────────────────────────

#[tauri::command]
pub async fn check_wallet_files(state: State<'_, AppState>) -> Result<Vec<WalletFileInfo>, String> {
    let dir = state.wallet_dir.read().await.clone();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("Failed to read wallet directory: {e}"))?;

    let mut wallets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "keys") {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());

            wallets.push(WalletFileInfo {
                name,
                path: path.to_string_lossy().to_string(),
                modified,
            });
        }
    }

    wallets.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(wallets)
}

#[tauri::command]
pub async fn init_wallet_rpc(state: State<'_, AppState>) -> Result<bool, String> {
    let wallet_url = state.wallet_url().await;

    // If wallet-rpc is already responding, skip spawn
    if wallet_process::port_is_occupied(&state.http, &wallet_url).await {
        return Ok(true);
    }

    let daemon_url = state.url().await;
    let wallet_dir = state.wallet_dir.read().await.clone();
    let network = *state.network.read().await;
    let rpc_port = network.default_wallet_rpc_port();

    let binary = wallet_process::resolve_binary(None)?;
    let child = wallet_process::spawn_wallet_rpc(&binary, &wallet_dir, &daemon_url, rpc_port)?;

    {
        let mut proc = state
            .wallet_process
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        *proc = Some(child);
    }

    wallet_process::wait_for_ready(&state.http, &wallet_url).await?;
    Ok(true)
}

#[tauri::command]
pub async fn shutdown_wallet_rpc(state: State<'_, AppState>) -> Result<bool, String> {
    let wallet_url = state.wallet_url().await;

    // Try graceful RPC stop before touching the mutex
    let _ = wallet_rpc::stop_wallet(&state.http, &wallet_url).await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    {
        let mut proc = state
            .wallet_process
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        if let Some(ref mut child) = *proc {
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        *proc = None;
    }

    *state.wallet_open.write().await = false;
    *state.wallet_name.write().await = None;
    Ok(true)
}

// ─── Wallet lifecycle commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn create_wallet(
    state: State<'_, AppState>,
    name: String,
    password: String,
    language: Option<String>,
) -> Result<CreateWalletResult, String> {
    let wallet_url = state.wallet_url().await;
    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());

    wallet_rpc::create_wallet(&state.http, &wallet_url, &name, &password, &lang).await?;

    let addr_resp = wallet_rpc::get_address(&state.http, &wallet_url, 0).await?;
    let seed = wallet_rpc::query_key(&state.http, &wallet_url, "mnemonic").await?;

    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(name.clone());

    Ok(CreateWalletResult {
        name,
        address: addr_resp.address,
        seed,
        seed_language: lang,
        network: network.as_str().into(),
    })
}

#[tauri::command]
pub async fn open_wallet(
    state: State<'_, AppState>,
    filename: String,
    password: String,
) -> Result<WalletInfo, String> {
    let wallet_url = state.wallet_url().await;
    let network = state.network.read().await;

    wallet_rpc::open_wallet(&state.http, &wallet_url, &filename, &password).await?;

    let addr_resp = wallet_rpc::get_address(&state.http, &wallet_url, 0).await?;

    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(filename.clone());

    Ok(WalletInfo {
        name: filename,
        address: addr_resp.address,
        seed_language: "English".into(),
        network: network.as_str().into(),
    })
}

#[tauri::command]
pub async fn close_wallet(state: State<'_, AppState>) -> Result<bool, String> {
    let wallet_url = state.wallet_url().await;
    wallet_rpc::close_wallet(&state.http, &wallet_url).await?;

    *state.wallet_open.write().await = false;
    *state.wallet_name.write().await = None;
    Ok(true)
}

#[tauri::command]
pub async fn import_wallet_from_seed(
    state: State<'_, AppState>,
    name: String,
    seed: String,
    password: String,
    language: Option<String>,
    restore_height: Option<u64>,
) -> Result<WalletInfo, String> {
    let wallet_url = state.wallet_url().await;
    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());
    let height = restore_height.unwrap_or(0);

    let resp = wallet_rpc::restore_deterministic_wallet(
        &state.http,
        &wallet_url,
        &name,
        &seed,
        &password,
        &lang,
        height,
        "",
    )
    .await?;

    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(name.clone());

    Ok(WalletInfo {
        name,
        address: resp.address,
        seed_language: lang,
        network: network.as_str().into(),
    })
}

#[tauri::command]
pub async fn import_wallet_from_keys(
    state: State<'_, AppState>,
    name: String,
    address: String,
    spendkey: String,
    viewkey: String,
    password: String,
    language: Option<String>,
    restore_height: Option<u64>,
) -> Result<WalletInfo, String> {
    let wallet_url = state.wallet_url().await;
    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());
    let height = restore_height.unwrap_or(0);

    let resp = wallet_rpc::generate_from_keys(
        &state.http,
        &wallet_url,
        &name,
        &address,
        &spendkey,
        &viewkey,
        &password,
        &lang,
        height,
    )
    .await?;

    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(name.clone());

    Ok(WalletInfo {
        name,
        address: resp.address,
        seed_language: lang,
        network: network.as_str().into(),
    })
}

#[tauri::command]
pub async fn get_seed(state: State<'_, AppState>) -> Result<String, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }
    let wallet_url = state.wallet_url().await;
    wallet_rpc::query_key(&state.http, &wallet_url, "mnemonic").await
}

// ─── Wallet data commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_balance(state: State<'_, AppState>) -> Result<Balance, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Ok(Balance {
            total: 0,
            unlocked: 0,
            staked: 0,
        });
    }

    let wallet_url = state.wallet_url().await;
    let resp = wallet_rpc::get_balance(&state.http, &wallet_url, 0).await?;

    Ok(Balance {
        total: resp.balance,
        unlocked: resp.unlocked_balance,
        staked: 0,
    })
}

#[tauri::command]
pub async fn get_address(
    state: State<'_, AppState>,
    account: u32,
    _index: u32,
) -> Result<String, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }

    let wallet_url = state.wallet_url().await;
    let resp = wallet_rpc::get_address(&state.http, &wallet_url, account).await?;
    Ok(resp.address)
}

#[tauri::command]
pub async fn transfer(
    state: State<'_, AppState>,
    address: String,
    amount: u64,
) -> Result<TxInfo, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }

    let wallet_url = state.wallet_url().await;
    let resp = wallet_rpc::transfer(&state.http, &wallet_url, &address, amount).await?;

    Ok(TxInfo {
        hash: resp.tx_hash,
        amount: resp.amount,
        fee: resp.fee,
        height: 0,
        timestamp: 0,
        direction: "out".into(),
        confirmed: false,
    })
}

#[tauri::command]
pub async fn get_transactions(
    state: State<'_, AppState>,
    _offset: u32,
    _limit: u32,
) -> Result<Vec<TxInfo>, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Ok(vec![]);
    }

    let wallet_url = state.wallet_url().await;
    let resp = wallet_rpc::get_transfers(&state.http, &wallet_url, true, true, true, false).await?;

    let mut txs: Vec<TxInfo> = Vec::new();
    for entry in resp.r#in {
        txs.push(TxInfo {
            hash: entry.txid,
            amount: entry.amount,
            fee: entry.fee,
            height: entry.height,
            timestamp: entry.timestamp,
            direction: "in".into(),
            confirmed: entry.confirmations > 0,
        });
    }
    for entry in resp.out {
        txs.push(TxInfo {
            hash: entry.txid,
            amount: entry.amount,
            fee: entry.fee,
            height: entry.height,
            timestamp: entry.timestamp,
            direction: "out".into(),
            confirmed: entry.confirmations > 0,
        });
    }
    for entry in resp.pending {
        txs.push(TxInfo {
            hash: entry.txid,
            amount: entry.amount,
            fee: entry.fee,
            height: 0,
            timestamp: entry.timestamp,
            direction: "out".into(),
            confirmed: false,
        });
    }

    txs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(txs)
}

#[tauri::command]
pub async fn get_staking_info() -> Result<serde_json::Value, String> {
    Err("Use get_chain_health for network staking data. Per-wallet staking requires wallet2 FFI bridge.".into())
}

#[tauri::command]
pub async fn stake(_tier: u8, _amount: u64) -> Result<TxInfo, String> {
    Err("Not yet implemented — requires wallet2 FFI bridge".into())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pqc_status_reports_hybrid() {
        let status = get_pqc_status().await.unwrap();
        assert!(status.enabled);
        assert_eq!(status.scheme, "Hybrid");
        assert_eq!(status.classical, "Ed25519");
        assert!(status.post_quantum.contains("ML-DSA"));
        assert_eq!(status.tx_version, 3);
    }

    #[tokio::test]
    async fn stake_returns_error_stub() {
        let err = stake(1, 5_000_000).await.unwrap_err();
        assert!(err.contains("wallet2 FFI"));
    }

    #[tokio::test]
    async fn get_staking_info_returns_ffi_message() {
        let err = get_staking_info().await.unwrap_err();
        assert!(err.contains("wallet2 FFI"));
    }
}
