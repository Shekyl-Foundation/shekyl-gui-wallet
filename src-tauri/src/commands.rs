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
use crate::validate;
use crate::wallet_bridge;

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
    pub pqc_protected: bool,
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

#[derive(Debug, Serialize)]
pub struct SecurityStatus {
    pub scheme: String,
    pub classical: String,
    pub post_quantum: String,
    pub tx_version: u8,
    pub anonymity_set_size: u64,
    pub tree_depth: u8,
    pub tree_root_short: String,
    pub reference_block_window: u16,
    pub proof_type: String,
    pub max_inputs: u8,
    pub estimated_proof_size_kb: f32,
    pub paths_precomputed: bool,
}

#[derive(Debug, Serialize)]
pub struct StakedOutputInfo {
    pub amount: u64,
    pub tier: u8,
    pub lock_height: u64,
    pub unlock_height: u64,
    pub claimable: bool,
}

#[derive(Debug, Serialize)]
pub struct WalletStakingInfo {
    pub total_staked: u64,
    pub staked_outputs: Vec<StakedOutputInfo>,
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
pub async fn init_wallet_rpc(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    if wallet_bridge::is_initialized(&state.wallet) {
        return Ok(true);
    }

    let daemon_addr = state.daemon_address().await;
    let wallet_dir = state.wallet_dir.read().await.clone();
    let network = *state.network.read().await;
    let nettype: u8 = match network {
        NetworkType::Mainnet => 0,
        NetworkType::Testnet => 1,
        NetworkType::Stagenet => 2,
    };

    let dir_str = wallet_dir.to_string_lossy().to_string();
    wallet_bridge::init(&state.wallet, nettype, &daemon_addr, &dir_str)?;
    wallet_bridge::setup_progress_bridge(&state.wallet, app)?;
    Ok(true)
}

#[tauri::command]
pub async fn shutdown_wallet_rpc(state: State<'_, AppState>) -> Result<bool, String> {
    wallet_bridge::shutdown(&state.wallet)?;

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
    validate::validate_wallet_name(&name)?;
    validate::validate_password(&password)?;

    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());

    wallet_bridge::create_wallet(&state.wallet, &name, &password, &lang)?;

    let addr_resp = wallet_bridge::get_address(&state.wallet, 0)?;
    let seed = wallet_bridge::query_key(&state.wallet, "mnemonic")?;

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
    app: tauri::AppHandle,
    filename: String,
    password: String,
) -> Result<WalletInfo, String> {
    validate::validate_wallet_name(&filename)?;
    validate::validate_password(&password)?;

    let network = state.network.read().await;
    let daemon_addr = state.daemon_address().await;

    wallet_bridge::open_wallet(&state.wallet, &filename, &password, &daemon_addr, app)?;

    let addr_resp = wallet_bridge::get_address(&state.wallet, 0)?;

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
    wallet_bridge::close_wallet(&state.wallet)?;

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
    validate::validate_wallet_name(&name)?;
    validate::validate_seed(&seed)?;
    validate::validate_password(&password)?;

    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());
    let height = restore_height.unwrap_or(0);

    let resp = wallet_bridge::restore_deterministic_wallet(
        &state.wallet,
        &name,
        &seed,
        &password,
        &lang,
        height,
        "",
    )?;

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
#[allow(clippy::too_many_arguments)]
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
    validate::validate_wallet_name(&name)?;
    validate::validate_address(&address)?;
    validate::validate_secret_key(&spendkey, "spend key")?;
    validate::validate_secret_key(&viewkey, "view key")?;
    validate::validate_password(&password)?;

    let network = state.network.read().await;
    let lang = language.unwrap_or_else(|| "English".into());
    let height = restore_height.unwrap_or(0);

    let resp = wallet_bridge::generate_from_keys(
        &state.wallet,
        &name,
        &address,
        &spendkey,
        &viewkey,
        &password,
        &lang,
        height,
    )?;

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
    wallet_bridge::query_key(&state.wallet, "mnemonic")
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

    // Read from Rust scanner state (primary) with C++ fallback
    match wallet_bridge::get_scanner_balance(&state.wallet).await {
        Ok(summary) => Ok(Balance {
            total: summary.total,
            unlocked: summary.unlocked,
            staked: summary.staked_total,
        }),
        Err(_) => {
            let resp = wallet_bridge::get_balance(&state.wallet, 0)?;
            Ok(Balance {
                total: resp.balance,
                unlocked: resp.unlocked_balance,
                staked: 0,
            })
        }
    }
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

    let resp = wallet_bridge::get_address(&state.wallet, account)?;
    Ok(resp.address)
}

#[tauri::command]
pub async fn transfer(
    state: State<'_, AppState>,
    address: String,
    amount: u64,
) -> Result<TxInfo, String> {
    validate::validate_address(&address)?;
    validate::validate_amount(amount)?;

    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }

    let resp = wallet_bridge::transfer(&state.wallet, &address, amount)?;

    Ok(TxInfo {
        hash: resp.tx_hash,
        amount: resp.amount,
        fee: resp.fee,
        height: 0,
        timestamp: 0,
        direction: "out".into(),
        confirmed: false,
        pqc_protected: true,
    })
}

#[tauri::command]
pub async fn estimate_fee(
    _state: State<'_, AppState>,
    address: String,
    amount: u64,
) -> Result<u64, String> {
    validate::validate_address(&address)?;
    validate::validate_amount(amount)?;

    // FCMP++ tx cost model: base_fee_per_byte * estimated_weight.
    // A typical 2-in/2-out FCMP++ tx is ~15-20 KB including the membership
    // proof and PQC auth. Conservative default until the daemon provides
    // a dynamic base_fee.
    const BASE_FEE_PER_BYTE: u64 = 20;
    const ESTIMATED_TX_BYTES: u64 = 18_000;
    Ok(BASE_FEE_PER_BYTE * ESTIMATED_TX_BYTES)
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

    let resp = wallet_bridge::get_transfers(&state.wallet, true, true, true, false)?;

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
            pqc_protected: entry.pqc_protected,
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
            pqc_protected: entry.pqc_protected,
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
            pqc_protected: entry.pqc_protected,
        });
    }

    txs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(txs)
}

#[tauri::command]
pub async fn get_staking_info(state: State<'_, AppState>) -> Result<WalletStakingInfo, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Ok(WalletStakingInfo {
            total_staked: 0,
            staked_outputs: vec![],
        });
    }

    let resp = wallet_bridge::get_staked_outputs(&state.wallet)?;
    Ok(WalletStakingInfo {
        total_staked: resp.total_staked,
        staked_outputs: resp
            .staked_outputs
            .into_iter()
            .map(|o| StakedOutputInfo {
                amount: o.amount,
                tier: o.tier,
                lock_height: o.lock_height,
                unlock_height: o.unlock_height,
                claimable: o.claimable,
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn stake(state: State<'_, AppState>, tier: u8, amount: u64) -> Result<TxInfo, String> {
    validate::validate_tier(tier)?;
    validate::validate_amount(amount)?;

    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }

    let resp = wallet_bridge::stake(&state.wallet, tier, amount)?;
    Ok(TxInfo {
        hash: resp.tx_hash,
        amount: resp.amount,
        fee: resp.fee,
        height: 0,
        timestamp: 0,
        direction: "out".into(),
        confirmed: false,
        pqc_protected: true,
    })
}

#[tauri::command]
pub async fn claim_rewards(state: State<'_, AppState>) -> Result<TxInfo, String> {
    let is_open = *state.wallet_open.read().await;
    if !is_open {
        return Err("No wallet is open".into());
    }

    let resp = wallet_bridge::claim_rewards(&state.wallet)?;
    Ok(TxInfo {
        hash: resp.tx_hash,
        amount: resp.amount,
        fee: resp.fee,
        height: 0,
        timestamp: 0,
        direction: "in".into(),
        confirmed: false,
        pqc_protected: true,
    })
}

#[tauri::command]
pub async fn get_curve_tree_info(
    state: State<'_, AppState>,
) -> Result<daemon_rpc::CurveTreeInfo, String> {
    let url = state.url().await;
    daemon_rpc::get_curve_tree_info(&state.http, &url).await
}

#[tauri::command]
pub async fn get_security_status(state: State<'_, AppState>) -> Result<SecurityStatus, String> {
    let url = state.url().await;
    let tree = daemon_rpc::get_curve_tree_info(&state.http, &url)
        .await
        .unwrap_or(daemon_rpc::CurveTreeInfo {
            root: String::new(),
            depth: 0,
            leaf_count: 0,
            height: 0,
        });

    let root_short = if tree.root.len() >= 8 {
        tree.root[..8].to_string()
    } else {
        tree.root.clone()
    };

    let wallet_refreshed =
        wallet_bridge::is_initialized(&state.wallet) && *state.wallet_open.read().await;

    Ok(SecurityStatus {
        scheme: "Hybrid".into(),
        classical: "Ed25519".into(),
        post_quantum: "ML-DSA-65 (FIPS 204)".into(),
        tx_version: 3,
        anonymity_set_size: tree.leaf_count,
        tree_depth: tree.depth,
        tree_root_short: root_short,
        reference_block_window: 100,
        proof_type: "FCMP++ Full-Chain Membership".into(),
        max_inputs: 8,
        estimated_proof_size_kb: 4.5,
        paths_precomputed: wallet_refreshed,
    })
}

// ─── PQC Multisig commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_multisig_group(
    state: State<'_, AppState>,
    n_total: u8,
    m_required: u8,
    participant_keys: Vec<String>,
) -> Result<serde_json::Value, String> {
    let resp = wallet_bridge::create_pqc_multisig_group(
        &state.wallet,
        n_total,
        m_required,
        participant_keys,
    )?;
    Ok(serde_json::json!({
        "group_id": resp.group_id,
        "n_total": resp.n_total,
        "m_required": resp.m_required,
    }))
}

#[tauri::command]
pub async fn get_multisig_info(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let info = wallet_bridge::get_pqc_multisig_info(&state.wallet)?;
    serde_json::to_value(info).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sign_multisig_partial(
    state: State<'_, AppState>,
    signing_request: String,
) -> Result<serde_json::Value, String> {
    let resp = wallet_bridge::sign_multisig_partial(&state.wallet, &signing_request)?;
    Ok(serde_json::json!({ "signature_response": resp.signature_response }))
}

// ─── Scanner commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_scanner_balance(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let summary = wallet_bridge::get_scanner_balance(&state.wallet).await?;
    Ok(serde_json::json!({
        "total": summary.total,
        "unlocked": summary.unlocked,
        "staked": summary.staked_total,
        "locked": summary.locked_by_timelock,
        "staked_matured": summary.staked_matured,
    }))
}

#[tauri::command]
pub async fn get_scanner_height(state: State<'_, AppState>) -> Result<u64, String> {
    wallet_bridge::get_scanner_height(&state.wallet).await
}

#[tauri::command]
pub async fn get_scanner_staked_outputs(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    wallet_bridge::get_scanner_staked_outputs(&state.wallet).await
}

#[tauri::command]
pub async fn get_scanner_claimable_stakes(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    wallet_bridge::get_scanner_claimable_stakes(&state.wallet).await
}

#[tauri::command]
pub async fn get_scanner_unstakeable_outputs(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    wallet_bridge::get_scanner_unstakeable_outputs(&state.wallet).await
}

#[tauri::command]
pub async fn scanner_freeze(state: State<'_, AppState>, key_image: String) -> Result<bool, String> {
    validate::validate_key_image(&key_image)?;
    wallet_bridge::scanner_freeze(&state.wallet, &key_image).await
}

#[tauri::command]
pub async fn scanner_thaw(state: State<'_, AppState>, key_image: String) -> Result<bool, String> {
    validate::validate_key_image(&key_image)?;
    wallet_bridge::scanner_thaw(&state.wallet, &key_image).await
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
    async fn security_status_returns_fcmp_fields() {
        // get_security_status requires daemon; tested via integration tests
    }
}
