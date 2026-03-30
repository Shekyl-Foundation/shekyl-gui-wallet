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
//! Commands that read chain/staking data call the daemon via JSON-RPC.
//! Wallet-side operations (create, open, transfer, stake, claim) remain
//! stubs until the wallet2 C++ FFI bridge is implemented.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::daemon_rpc;
use crate::state::{AppState, NetworkType};

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

    match daemon_rpc::get_info(&state.http, &url).await {
        Ok(info) => Ok(WalletStatus {
            connected: true,
            wallet_open: false,
            wallet_name: None,
            daemon_address: Some(url),
            network: network.as_str().into(),
            synced: info.synchronized,
            sync_height: info.height,
            daemon_height: info.target_height,
        }),
        Err(_) => Ok(WalletStatus {
            connected: false,
            wallet_open: false,
            wallet_name: None,
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

// ─── Wallet stubs (require wallet2 FFI bridge) ──────────────────────────────

#[tauri::command]
pub async fn create_wallet(name: String, _password: String) -> Result<WalletInfo, String> {
    Ok(WalletInfo {
        name,
        address: "SKL1mock...placeholder".into(),
        seed_language: "English".into(),
        network: "mainnet".into(),
    })
}

#[tauri::command]
pub async fn open_wallet(_path: String, _password: String) -> Result<WalletInfo, String> {
    Ok(WalletInfo {
        name: "Mock Wallet".into(),
        address: "SKL1mock...placeholder".into(),
        seed_language: "English".into(),
        network: "mainnet".into(),
    })
}

#[tauri::command]
pub async fn close_wallet() -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub async fn get_balance() -> Result<Balance, String> {
    Ok(Balance {
        total: 0,
        unlocked: 0,
        staked: 0,
    })
}

#[tauri::command]
pub async fn get_address(account: u32, index: u32) -> Result<String, String> {
    Ok(format!(
        "SKL1mock_account{}_subaddr{}...placeholder",
        account, index
    ))
}

#[tauri::command]
pub async fn transfer(_address: String, _amount: u64) -> Result<TxInfo, String> {
    Err("Not yet implemented — requires wallet2 FFI bridge".into())
}

#[tauri::command]
pub async fn get_transactions(_offset: u32, _limit: u32) -> Result<Vec<TxInfo>, String> {
    Ok(vec![])
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
    async fn create_wallet_returns_provided_name() {
        let info = create_wallet("Test".into(), "pass".into()).await.unwrap();
        assert_eq!(info.name, "Test");
        assert_eq!(info.network, "mainnet");
        assert_eq!(info.seed_language, "English");
        assert!(!info.address.is_empty());
    }

    #[tokio::test]
    async fn open_wallet_returns_mock_wallet() {
        let info = open_wallet("/tmp/w".into(), "pass".into()).await.unwrap();
        assert_eq!(info.name, "Mock Wallet");
        assert!(!info.address.is_empty());
    }

    #[tokio::test]
    async fn close_wallet_returns_true() {
        assert!(close_wallet().await.unwrap());
    }

    #[tokio::test]
    async fn balance_defaults_to_zero() {
        let b = get_balance().await.unwrap();
        assert_eq!(b.total, 0);
        assert_eq!(b.unlocked, 0);
        assert_eq!(b.staked, 0);
    }

    #[tokio::test]
    async fn get_address_encodes_account_and_index() {
        let addr = get_address(1, 2).await.unwrap();
        assert!(addr.contains("account1"));
        assert!(addr.contains("subaddr2"));
    }

    #[tokio::test]
    async fn transfer_returns_error_stub() {
        let err = transfer("SKL1abc".into(), 1_000_000).await.unwrap_err();
        assert!(err.contains("wallet2 FFI"));
    }

    #[tokio::test]
    async fn get_transactions_returns_empty_vec() {
        let txs = get_transactions(0, 10).await.unwrap();
        assert!(txs.is_empty());
    }

    #[tokio::test]
    async fn stake_returns_error_stub() {
        let err = stake(1, 5_000_000).await.unwrap_err();
        assert!(err.contains("wallet2 FFI"));
    }

    #[tokio::test]
    async fn pqc_status_reports_hybrid() {
        let status = get_pqc_status().await.unwrap();
        assert!(status.enabled);
        assert_eq!(status.scheme, "Hybrid");
        assert_eq!(status.classical, "Ed25519");
        assert!(status.post_quantum.contains("ML-DSA"));
        assert_eq!(status.tx_version, 3);
    }
}
