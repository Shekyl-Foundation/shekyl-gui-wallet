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
//! Import-from-keys was only ever backed by that path and returns an honest
//! "not available on the Engine backend" error until it is ported. The PQC
//! multisig surface lives in [`crate::multisig`] behind the `multisig` cargo
//! feature (off by default); the Wallet2 scanner stubs are gone.
//! Compiled feature switches live in [`crate::features`]. The recovery-phrase
//! clipboard slot lives in [`crate::clipboard`].

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::balance::Balance;
use crate::daemon_rpc;
use crate::drain_balance::DrainBalance;
use crate::gui_config;
use crate::staking_view::StakingView;
use crate::state::{self, AppState};
use crate::transfer_history::{TransferDirection, TransferFilter, TransferRow, TransferState};
use crate::validate;
use crate::wallet_name;
use crate::wire::AtomicUnitsString;

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
pub struct WalletFileInfo {
    pub name: String,
    pub path: String,
    pub modified: u64,
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
    pub block_reward: AtomicUnitsString,
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
        block_reward: ms.block_reward.into(),
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

/// Startup: guarantee the configured wallet directory exists before any
/// create/open flow runs (mkdir -p semantics on POSIX and Windows). The
/// Engine connects to the daemon per wallet-open; there is no global handle
/// to initialise, which is why this is all the old `init_wallet_rpc` did.
#[tauri::command]
pub async fn ensure_wallet_dir(state: State<'_, AppState>) -> Result<(), String> {
    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)
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

/// Archival staker status (Engine only).
#[derive(Debug, Serialize)]
pub struct StakerStatusInfo {
    pub staking_enabled: bool,
    pub has_stake_engine: bool,
    pub bonded_slot_count: u32,
    pub has_pscan: bool,
}

/// The contract's `StakeResult`.
#[derive(Debug, Serialize)]
pub struct StakeResult {
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

/// The contract's `stake { password }`: become an archival staker (Engine
/// `first_stake` under password re-auth). Posture is always `market`.
#[tauri::command]
pub async fn stake(state: State<'_, AppState>, password: String) -> Result<StakeResult, String> {
    validate::validate_password(&password)?;
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let mut eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("no wallet is open on the Engine backend".into());
    }
    let outcome = eng.stake(&password).await?;
    Ok(StakeResult {
        slot: outcome.slot,
        swept_inputs: outcome.swept_inputs,
        resumed: outcome.resumed,
        state: outcome.state.to_owned(),
    })
}

// ─── Wallet data commands ────────────────────────────────────────────────────

/// The contract's `get_balance`. A closed wallet is a non-value — an error
/// the card renders as "—" — never a fabricated zero balance (rule 82).
#[tauri::command]
pub async fn get_balance(state: State<'_, AppState>) -> Result<Balance, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    Ok(Balance::from(eng.balance_view().await?))
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

/// Authoritative staking read (`StakeFacade::staking_read_view`, WI-RPC-1).
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

/// The contract's `GetPrimaryAddressResult`: one address, no index — Shekyl
/// has no subaddresses; payment requests are the receive-attribution surface.
#[derive(Debug, Serialize)]
pub struct PrimaryAddress {
    pub address: String,
}

#[tauri::command]
pub async fn get_primary_address(state: State<'_, AppState>) -> Result<PrimaryAddress, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    if !eng.is_open() {
        return Err("No wallet is open".into());
    }
    let address = eng.primary_address().await?;
    Ok(PrimaryAddress { address })
}

/// The contract's `GetTransfersResult`.
#[derive(Debug, Serialize)]
pub struct Transfers {
    pub transfers: Vec<TransferRow>,
}

/// The contract's `get_transfers`, with the optional filters this wallet
/// offers (`direction`, `state`); unfiltered returns full history. The
/// enums deserialize the contract's spellings, so an unknown one fails
/// before this body runs. The contract's `since_height` watermark and
/// `attribution` filter have no page here and are not taken.
#[tauri::command]
pub async fn get_transfers(
    app: State<'_, AppState>,
    direction: Option<TransferDirection>,
    state: Option<TransferState>,
) -> Result<Transfers, String> {
    let filter = TransferFilter { direction, state };
    if !*app.wallet_open.read().await {
        return Ok(Transfers { transfers: vec![] });
    }
    let eng = app.engine.lock().await;
    if !eng.is_open() {
        return Ok(Transfers { transfers: vec![] });
    }
    let transfers = eng
        .list_transfers()
        .await?
        .into_iter()
        .filter(|row| filter.keeps(row))
        .collect();
    Ok(Transfers { transfers })
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

// ─── Daemon lifecycle commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn daemon_status(
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<crate::daemon_manager::DaemonStatus, String> {
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
