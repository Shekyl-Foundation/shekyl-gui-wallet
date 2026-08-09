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
//! Chain/staking/mining commands call the daemon via JSON-RPC. The wallet
//! lifecycle runs entirely on the pure-Rust [`crate::engine_session`] backend
//! — the transitional Wallet2 / `shekyl-engine-rpc` path has been removed.
//! Features that were only ever backed by that path (import-from-keys, PQC
//! multisig, scanner freeze/thaw) return honest "not available on the Engine
//! backend" errors until they are ported.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::daemon_rpc;
use crate::drain_balance::DrainBalance;
use crate::engine_session;
use crate::gui_config;
use crate::staking_view::StakingView;
use crate::state::{self, AppState, NetworkType};
use crate::transfer_history::{TransferDirection, TransferRow, TransferStatus};
use crate::validate;
use crate::wallet_name;

/// User-facing refusal for wallet features that only ran on the retired
/// Wallet2 backend and have no Engine implementation yet.
const ENGINE_BACKEND_UNSUPPORTED: &str = "\
this feature is not available on the Engine backend yet; it ran only on the \
retired Wallet2 path and is pending an Engine implementation";

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

/// Transaction list / send result row for the frontend.
///
/// Mirrors [`TransferRow`] on the wire. Settlement is expressed only via
/// [`TransferStatus`] — there is no parallel `confirmed` bool.
#[derive(Debug, Serialize)]
pub struct TxInfo {
    /// Stable list key (`hash:index` for receives, bare hash for sends).
    pub id: String,
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    /// Inclusion height, or `null` when the tx is not on chain.
    pub height: Option<u64>,
    pub timestamp: u64,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub pqc_protected: bool,
}

impl From<TransferRow> for TxInfo {
    fn from(r: TransferRow) -> Self {
        Self {
            id: r.id,
            hash: r.hash,
            amount: r.amount,
            fee: r.fee,
            height: r.height,
            timestamp: r.timestamp,
            direction: r.direction,
            status: r.status,
            pqc_protected: r.pqc_protected,
        }
    }
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
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        // Engine wallets are keyed by their `{name}.wallet.keys` envelope.
        let Some(stem) = fname.strip_suffix(".wallet.keys") else {
            continue;
        };
        let name = stem.to_string();
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

    wallets.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(wallets)
}

#[tauri::command]
pub async fn init_wallet_rpc(state: State<'_, AppState>) -> Result<bool, String> {
    // The Engine backend connects to the daemon per wallet-open (no global
    // handle to initialise). This retains its startup contract of guaranteeing
    // the configured wallet directory exists before any create/open flow runs
    // (mkdir -p semantics on POSIX and Windows).
    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;
    Ok(true)
}

/// Override the wallet directory with a user-chosen path (Advanced
/// directory picker in the create/import UI). Ensures the directory
/// exists before swapping it in; returns the canonical display string
/// that the UI can show back to the user. The choice is persisted to
/// `gui-config.json` so the next launch defaults to it.
#[tauri::command]
pub async fn set_wallet_dir(state: State<'_, AppState>, dir: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(&dir);
    if path.as_os_str().is_empty() {
        return Err("Wallet directory must not be empty".into());
    }
    wallet_name::ensure_dir_exists(&path)?;
    let display = path.to_string_lossy().to_string();
    gui_config::save(&gui_config::GuiConfig {
        schema_version: gui_config::SCHEMA_VERSION,
        wallet_dir_override: Some(path.clone()),
    });
    *state.wallet_dir.write().await = path;
    // A successful explicit choice clears any stale "fell back from"
    // warning the user might still be looking at.
    *state.wallet_dir_warning.write().await = None;
    Ok(display)
}

/// Reset the wallet directory to the platform default and clear any
/// persisted override.
#[tauri::command]
pub async fn reset_wallet_dir(state: State<'_, AppState>) -> Result<String, String> {
    let default = state::default_wallet_dir();
    wallet_name::ensure_dir_exists(&default)?;
    let display = default.to_string_lossy().to_string();
    gui_config::save(&gui_config::GuiConfig {
        schema_version: gui_config::SCHEMA_VERSION,
        wallet_dir_override: None,
    });
    *state.wallet_dir.write().await = default;
    *state.wallet_dir_warning.write().await = None;
    Ok(display)
}

/// Response shape for [`get_wallet_dir`].
///
/// `fallback_from` is `Some(path)` when the persisted wallet-dir
/// override was unreachable at startup and we silently fell back to
/// the platform default; the UI uses this to surface a "your custom
/// location is unavailable, using default" banner.
#[derive(Debug, Serialize)]
pub struct WalletDirResponse {
    pub dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_from: Option<String>,
}

/// Return the currently configured wallet directory plus a soft
/// warning if it was reached via fallback (override unreachable).
#[tauri::command]
pub async fn get_wallet_dir(state: State<'_, AppState>) -> Result<WalletDirResponse, String> {
    let dir = state.wallet_dir.read().await.clone();
    let fallback = state.wallet_dir_warning.read().await.clone();
    Ok(WalletDirResponse {
        dir: dir.to_string_lossy().to_string(),
        fallback_from: fallback.map(|p| p.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub async fn shutdown_wallet_rpc(state: State<'_, AppState>) -> Result<bool, String> {
    let close_result = {
        let mut eng = state.engine.lock().await;
        eng.close().await
    };
    // Clear the open flags even if close errored: the wallet is being torn
    // down, so the UI must not keep believing one is open — a stale
    // `wallet_open` would block a clean re-open.
    *state.wallet_open.write().await = false;
    *state.wallet_name.write().await = None;
    close_result?;
    Ok(true)
}

#[tauri::command]
pub async fn refresh_wallet(state: State<'_, AppState>) -> Result<bool, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    eng.refresh().await?;
    Ok(true)
}

/// Archival staker status (Engine only).
#[derive(Debug, Serialize)]
pub struct StakerStatusInfo {
    pub staking_enabled: bool,
    pub has_stake_engine: bool,
    pub bonded_slot_count: u32,
    pub has_pscan: bool,
}

/// Result of archival first-stake activation.
#[derive(Debug, Serialize)]
pub struct ActivateStakerResult {
    pub slot: u32,
    pub swept_inputs: usize,
    pub resumed: bool,
    pub state: String,
}

#[tauri::command]
pub async fn get_staker_status(state: State<'_, AppState>) -> Result<StakerStatusInfo, String> {
    if *state.wallet_open.read().await {
        let eng = state.engine.lock().await;
        if eng.is_open() {
            let s = eng.staker_status().await?;
            return Ok(StakerStatusInfo {
                staking_enabled: s.staking_enabled,
                has_stake_engine: s.has_stake_engine,
                bonded_slot_count: s.bonded_slot_count,
                has_pscan: s.has_pscan,
            });
        }
    }
    Ok(StakerStatusInfo {
        staking_enabled: false,
        has_stake_engine: false,
        bonded_slot_count: 0,
        has_pscan: false,
    })
}

/// Become an archival staker (Engine `first_stake` / password re-auth).
#[tauri::command]
pub async fn activate_staker(
    state: State<'_, AppState>,
    password: String,
) -> Result<ActivateStakerResult, String> {
    validate::validate_password(&password)?;
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let mut eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("no wallet is open on the Engine backend".into());
    }
    let outcome = eng.activate_staker(&password).await?;
    Ok(ActivateStakerResult {
        slot: outcome.slot,
        swept_inputs: outcome.swept_inputs,
        resumed: outcome.resumed,
        state: outcome.state.to_owned(),
    })
}

// ─── Wallet lifecycle commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn create_wallet(
    state: State<'_, AppState>,
    name: String,
    password: String,
    language: Option<String>,
) -> Result<CreateWalletResult, String> {
    // Sanitize first (collapses whitespace, replaces spaces with '_') so
    // the on-disk name is filesystem-friendly regardless of what the
    // user typed.
    let sanitized = wallet_name::sanitize(&name);
    validate::validate_wallet_name(&sanitized)?;
    validate::validate_password(&password)?;

    // `language` is a legacy Wallet2 mnemonic-language selector; the Engine
    // derives the recovery phrase itself (BIP-39 English on mainnet/stagenet,
    // raw32 hex on testnet), so the argument is accepted for API stability but
    // no longer drives seed generation.
    let _ = language;
    let network = *state.network.read().await;

    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    let outcome = eng
        .create(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(CreateWalletResult {
        name: sanitized,
        address: outcome.address,
        seed: outcome.seed,
        seed_language: seed_language_for(network),
        network: network.as_str().into(),
    })
}

/// The recovery-phrase encoding the Engine uses for a freshly created or
/// opened wallet on `network`: BIP-39 English everywhere except testnet,
/// which uses a raw 32-byte hex seed.
fn seed_language_for(network: NetworkType) -> String {
    if network == NetworkType::Testnet {
        "raw32".into()
    } else {
        "BIP-39 English".into()
    }
}

#[tauri::command]
pub async fn open_wallet(
    state: State<'_, AppState>,
    filename: String,
    password: String,
) -> Result<WalletInfo, String> {
    validate::validate_password(&password)?;

    let network = *state.network.read().await;
    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let sanitized = wallet_name::sanitize(&filename);
    validate::validate_wallet_name(&sanitized)?;

    if !engine_session::engine_wallet_exists(&wallet_dir, &sanitized) {
        return Err(format!(
            "no wallet found for '{sanitized}' (expected {sanitized}.wallet.keys)"
        ));
    }

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    let address = eng
        .open(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    // Best-effort tip catch-up (non-fatal).
    if let Err(e) = eng.refresh().await {
        tracing::warn!(error = %e, "engine refresh after open failed");
    }
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(WalletInfo {
        name: sanitized,
        address,
        seed_language: seed_language_for(network),
        network: network.as_str().into(),
    })
}

#[tauri::command]
pub async fn close_wallet(state: State<'_, AppState>) -> Result<bool, String> {
    let close_result = {
        let mut eng = state.engine.lock().await;
        if eng.is_open() {
            eng.close().await
        } else {
            Ok(())
        }
    };
    // Clear the open flags even if close errored, so the UI reflects the
    // teardown; the close error is then surfaced rather than swallowed.
    *state.wallet_open.write().await = false;
    *state.wallet_name.write().await = None;
    close_result?;
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
    let sanitized = wallet_name::sanitize(&name);
    validate::validate_wallet_name(&sanitized)?;
    validate::validate_recovery_phrase(&seed)?;
    validate::validate_password(&password)?;

    // `language` is a legacy Wallet2 mnemonic-language selector; the Engine
    // restores from the BIP-39 phrase directly, so it is accepted for API
    // stability but no longer used.
    let _ = language;
    let network = *state.network.read().await;
    let height = restore_height.unwrap_or(0);

    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    let address = eng
        .restore_from_bip39(
            &wallet_dir,
            &sanitized,
            &seed,
            &password,
            "",
            height,
            network,
            &daemon,
        )
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(WalletInfo {
        name: sanitized,
        address,
        seed_language: seed_language_for(network),
        network: network.as_str().into(),
    })
}

/// Import from raw view/spend keys.
///
/// Retired with the Wallet2 backend: the Engine has no key-import path yet, so
/// this returns an honest refusal rather than silently doing nothing. Inputs
/// are still validated so the UI surfaces malformed keys the same way.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn import_wallet_from_keys(
    _state: State<'_, AppState>,
    name: String,
    address: String,
    spendkey: String,
    viewkey: String,
    password: String,
    _language: Option<String>,
    _restore_height: Option<u64>,
) -> Result<WalletInfo, String> {
    validate::validate_wallet_name(&wallet_name::sanitize(&name))?;
    validate::validate_address(&address)?;
    validate::validate_secret_key(&spendkey, "spend key")?;
    validate::validate_secret_key(&viewkey, "view key")?;
    validate::validate_password(&password)?;
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn get_seed(state: State<'_, AppState>) -> Result<String, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let mut eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    if let Some(m) = eng.take_create_mnemonic() {
        return Ok(m);
    }
    Err(engine_session::EngineSession::seed_unavailable_message().into())
}

// ─── Wallet data commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_balance(state: State<'_, AppState>) -> Result<Balance, String> {
    if !*state.wallet_open.read().await {
        return Ok(Balance {
            total: 0,
            unlocked: 0,
            staked: 0,
        });
    }

    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Ok(Balance {
            total: 0,
            unlocked: 0,
            staked: 0,
        });
    }
    // `staked` is reported as 0 by design: personal archival stake is shown
    // only on the Staking page (WI-RPC-1 three-leg view), not as a single
    // dashboard total. See `EngineSession::balance` dual-truth note.
    let (total, unlocked, staked) = eng.balance().await?;
    Ok(Balance {
        total,
        unlocked,
        staked,
    })
}

/// F-D2 aggregate drainable-`P` read (DS-PR-3 PR-B). Staker-only figure.
///
/// Returns the single wire DTO from [`crate::drain_balance`] — no second
/// identity map. A closed / not-yet-open wallet is a *non-value*, not a zero:
/// it returns `Err("No wallet is open")` so the frontend `.catch` renders
/// "—". An *open* wallet with no P-scan seal is an honest
/// `Ready { spendable: 0 }`. Transient anchor lag surfaces as `Syncing`; a
/// non-transient read fault propagates as `Err(String)`.
#[tauri::command]
pub async fn get_drain_balance(state: State<'_, AppState>) -> Result<DrainBalance, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    eng.drain_balance().await
}

/// Authoritative staking read (Engine `staking_read_view`, WI-RPC-1).
///
/// Returns the single wire DTO from [`crate::staking_view`] — no second
/// identity map. Fail-closed like `get_drain_balance`: a closed wallet and a
/// corrupt / version-mismatched seal are both `Err(String)` — the frontend
/// `.catch` renders a non-value, never "nothing staked" over a bad read
/// (rule 82). An open non-staker wallet is an honest all-zero / empty view
/// from the core.
#[tauri::command]
pub async fn get_staking_view(state: State<'_, AppState>) -> Result<StakingView, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    eng.staking_view().await
}

#[tauri::command]
pub async fn get_address(
    state: State<'_, AppState>,
    account: u32,
    _index: u32,
) -> Result<String, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    if account != 0 {
        return Err("Engine backend: only the primary account is supported".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    eng.primary_address().await
}

#[tauri::command]
pub async fn transfer(
    state: State<'_, AppState>,
    address: String,
    amount: u64,
) -> Result<TxInfo, String> {
    validate::validate_address(&address)?;
    validate::validate_amount(amount)?;

    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    let outcome = eng.transfer(&address, amount).await?;
    Ok(TxInfo {
        id: outcome.tx_hash.clone(),
        hash: outcome.tx_hash,
        amount: outcome.amount,
        fee: outcome.fee,
        height: None,
        timestamp: 0,
        direction: TransferDirection::Out,
        status: TransferStatus::Pending,
        pqc_protected: true,
    })
}

#[tauri::command]
pub async fn estimate_fee(
    state: State<'_, AppState>,
    address: String,
    amount: u64,
) -> Result<u64, String> {
    validate::validate_address(&address)?;
    validate::validate_amount(amount)?;

    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    eng.estimate_fee(&address, amount).await
}

#[tauri::command]
pub async fn get_transactions(
    state: State<'_, AppState>,
    _offset: u32,
    _limit: u32,
) -> Result<Vec<TxInfo>, String> {
    if !*state.wallet_open.read().await {
        return Ok(vec![]);
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Ok(vec![]);
    }
    let rows = eng.list_transfers().await?;
    Ok(rows.into_iter().map(TxInfo::from).collect())
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

    let wallet_refreshed = *state.wallet_open.read().await;

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

// PQC multisig ran only on the Wallet2 backend, which has been retired. These
// commands stay registered so the Multisig page loads, but return an honest
// refusal until multisig is ported to the Engine backend.

#[tauri::command]
pub async fn create_multisig_group(
    _state: State<'_, AppState>,
    _n_total: u8,
    _m_required: u8,
    _participant_keys: Vec<String>,
) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn get_multisig_info(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn sign_multisig_partial(
    _state: State<'_, AppState>,
    _signing_request: String,
) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

// ─── Group Descriptor import/export ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupDescriptorPayload {
    pub version: u8,
    pub group_id: String,
    pub m_required: u8,
    pub n_total: u8,
    pub spend_auth_version: u8,
    pub participant_pubkeys: Vec<String>,
    pub address_fingerprint: String,
    pub relays: Vec<GroupDescriptorRelay>,
    pub created_at: u64,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupDescriptorRelay {
    pub url: String,
    pub operator_id: String,
}

#[tauri::command]
pub async fn export_group_descriptor(
    _state: State<'_, AppState>,
    _path: String,
) -> Result<(), String> {
    // Depends on multisig group info, which is Wallet2-only and retired.
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn import_group_descriptor(path: String) -> Result<GroupDescriptorPayload, String> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read descriptor file: {e}"))?;
    let desc: GroupDescriptorPayload =
        serde_json::from_str(&json).map_err(|e| format!("Invalid descriptor format: {e}"))?;

    if desc.version != 1 {
        return Err(format!("Unsupported descriptor version: {}", desc.version));
    }
    if desc.m_required == 0 || desc.m_required > desc.n_total {
        return Err(format!(
            "Invalid threshold: {}-of-{}",
            desc.m_required, desc.n_total
        ));
    }
    if desc.participant_pubkeys.len() != desc.n_total as usize {
        return Err(format!(
            "Expected {} pubkeys, got {}",
            desc.n_total,
            desc.participant_pubkeys.len()
        ));
    }

    Ok(desc)
}

// ─── File-based transport ────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_signing_request_file(
    state: State<'_, AppState>,
    signing_request: String,
    path: String,
) -> Result<(), String> {
    let _ = &state;
    std::fs::write(&path, signing_request.as_bytes())
        .map_err(|e| format!("Failed to write signing request: {e}"))
}

#[tauri::command]
pub async fn import_signing_request_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read signing request: {e}"))
}

#[tauri::command]
pub async fn export_signature_response_file(response: String, path: String) -> Result<(), String> {
    std::fs::write(&path, response.as_bytes())
        .map_err(|e| format!("Failed to write signature response: {e}"))
}

// ─── Scanner commands ─────────────────────────────────────────────────────────
//
// The scanner surfaced here was the Wallet2 in-process sync loop, now retired.
// The Engine owns its own ledger/balance (see `get_balance`), so these
// commands return an honest refusal until an Engine-native equivalent is
// exposed. `scanner_freeze` / `scanner_thaw` still validate their key image so
// the UI surfaces malformed input the same way. (The staked-output query
// stubs are gone: `get_staking_view` is their Engine-native replacement,
// GUI-PR3b.)

#[tauri::command]
pub async fn get_scanner_balance(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn get_scanner_height(_state: State<'_, AppState>) -> Result<u64, String> {
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn scanner_freeze(
    _state: State<'_, AppState>,
    key_image: String,
) -> Result<bool, String> {
    validate::validate_key_image(&key_image)?;
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

#[tauri::command]
pub async fn scanner_thaw(_state: State<'_, AppState>, key_image: String) -> Result<bool, String> {
    validate::validate_key_image(&key_image)?;
    Err(ENGINE_BACKEND_UNSUPPORTED.into())
}

// ─── Daemon lifecycle commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn daemon_status(
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<crate::daemon_manager::DaemonStatus, String> {
    Ok(dm.status().await)
}

#[tauri::command]
pub async fn restart_daemon(
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
    app: tauri::AppHandle,
) -> Result<crate::daemon_manager::DaemonStatus, String> {
    dm.shutdown().await;
    dm.start(&app).await;
    Ok(dm.status().await)
}

#[tauri::command]
pub async fn get_daemon_settings(
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<crate::daemon_manager::DaemonConfig, String> {
    Ok(dm.config().await)
}

#[tauri::command]
pub async fn set_daemon_settings(
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
    keep_running_on_exit: Option<bool>,
    data_dir: Option<String>,
    rpc_port: Option<u16>,
) -> Result<crate::daemon_manager::DaemonConfig, String> {
    let mut config = dm.config().await;
    if let Some(v) = keep_running_on_exit {
        config.keep_running_on_exit = v;
    }
    if let Some(v) = data_dir {
        config.data_dir = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(v) = rpc_port {
        config.rpc_port = v;
    }
    dm.update_config(config.clone()).await?;
    Ok(config)
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
        // get_security_status requires a running daemon connection.
        // Tested in CI integration tests with a regtest daemon.
        // Here we verify the PqcStatus struct (which is daemon-independent)
        // is consistent with the security status expectations.
        let pqc = get_pqc_status().await.unwrap();
        assert!(pqc.enabled, "PQC should always be enabled");
        assert_eq!(pqc.tx_version, 3, "Shekyl is v3-from-genesis");
        assert!(
            pqc.post_quantum.contains("ML-DSA") || pqc.post_quantum.contains("Dilithium"),
            "post_quantum field should name the PQC scheme"
        );
    }
}
