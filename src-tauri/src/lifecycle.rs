// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Wallet lifecycle — `create_wallet`, `open_wallet`, `close_wallet`,
//! `restore_wallet` — under the wallet contract's names and result shapes.
//! A feature module (rule 27) over `EngineSession`, which does the opening.
//!
//! Every call that leaves a wallet open returns the contract's
//! `WalletHandle` (name, the envelope's own capability, network, the
//! restore-height hint the file carries) and nothing else: the address is
//! read from `get_primary_address`, its one source. `create_wallet` adds the
//! backup exactly once, in the network's encoding — `mnemonic` on
//! mainnet/stagenet, `raw_seed_hex` on testnet — and the session keeps no
//! copy of it.

use serde::Serialize;
use shekyl_engine_core::Capability;
use tauri::State;

use crate::engine_session;
use crate::state::{AppState, NetworkType};
use crate::validate;
use crate::wallet_name;

/// The contract's `WalletHandle`: returned by every lifecycle call that
/// leaves a wallet open. Informational, not a bearer token.
#[derive(Debug, Serialize)]
pub struct WalletHandle {
    /// Wallet file stem within the wallet directory.
    pub name: String,
    /// `CapabilityMode` — the envelope's own capability, never inferred.
    pub capability: &'static str,
    /// `MAINNET | TESTNET | STAGENET`.
    pub network: &'static str,
    /// The rescan floor the wallet file carries; absent when it scans from genesis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_height_hint: Option<u64>,
}

/// `open_wallet` / `restore_wallet`: the contract's `OpenWalletResult` /
/// `RestoreWalletResult`. No address here — `get_primary_address` is the
/// one source for it.
#[derive(Debug, Serialize)]
pub struct OpenedWallet {
    pub wallet: WalletHandle,
}

/// `create_wallet`: the contract's `CreateWalletResult`. Backup material is
/// returned exactly once, in the network's encoding: `mnemonic` on
/// mainnet/stagenet, `raw_seed_hex` on testnet, never both.
#[derive(Debug, Serialize)]
pub struct CreatedWallet {
    pub wallet: WalletHandle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_seed_hex: Option<String>,
}

/// The contract's spellings for a handle.
pub mod contract {
    pub const CAPABILITY_FULL: &str = "FULL";
    pub const NETWORK_MAINNET: &str = "MAINNET";
    pub const NETWORK_TESTNET: &str = "TESTNET";
    pub const NETWORK_STAGENET: &str = "STAGENET";
}

fn network_str(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Mainnet => contract::NETWORK_MAINNET,
        NetworkType::Testnet => contract::NETWORK_TESTNET,
        NetworkType::Stagenet => contract::NETWORK_STAGENET,
    }
}

fn capability_str(capability: Capability) -> &'static str {
    match capability {
        Capability::Full => contract::CAPABILITY_FULL,
    }
}

/// Build the handle for the wallet the session just opened, from the
/// engine's own facts.
async fn wallet_handle(
    eng: &engine_session::EngineSession,
    name: String,
    network: NetworkType,
) -> Result<WalletHandle, String> {
    let (capability, restore_height_hint) = eng.handle_facts().await?;
    Ok(WalletHandle {
        name,
        capability: capability_str(capability),
        network: network_str(network),
        restore_height_hint: (restore_height_hint > 0).then_some(u64::from(restore_height_hint)),
    })
}

#[tauri::command]
pub async fn create_wallet(
    state: State<'_, AppState>,
    name: String,
    password: String,
) -> Result<CreatedWallet, String> {
    // Sanitize first (collapses whitespace, replaces spaces with '_') so
    // the on-disk name is filesystem-friendly regardless of what the
    // user typed.
    let sanitized = wallet_name::sanitize(&name);
    validate::validate_wallet_name(&sanitized)?;
    validate::validate_password(&password)?;

    let network = *state.network.read().await;

    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    let backup = eng
        .create(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    let wallet = wallet_handle(&eng, sanitized, network).await?;
    let (mnemonic, raw_seed_hex) = match backup {
        engine_session::SeedBackup::Mnemonic(m) => (Some(m), None),
        engine_session::SeedBackup::RawHex(h) => (None, Some(h)),
    };
    Ok(CreatedWallet {
        wallet,
        mnemonic,
        raw_seed_hex,
    })
}

#[tauri::command]
pub async fn open_wallet(
    state: State<'_, AppState>,
    filename: String,
    password: String,
) -> Result<OpenedWallet, String> {
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
    eng.open(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(OpenedWallet {
        wallet: wallet_handle(&eng, sanitized, network).await?,
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

/// The contract's `restore_wallet` (`mnemonic` is the seed backup in the
/// network's encoding — see `validate_seed_backup`).
#[tauri::command]
pub async fn restore_wallet(
    state: State<'_, AppState>,
    name: String,
    password: String,
    mnemonic: String,
    restore_height: Option<u64>,
) -> Result<OpenedWallet, String> {
    let sanitized = wallet_name::sanitize(&name);
    let network = *state.network.read().await;
    validate::validate_wallet_name(&sanitized)?;
    validate::validate_seed_backup(&mnemonic, network)?;
    validate::validate_password(&password)?;

    let height = restore_height.unwrap_or(0);

    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    eng.restore_from_backup(
        &wallet_dir,
        &sanitized,
        &mnemonic,
        &password,
        height,
        network,
        &daemon,
    )
    .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(OpenedWallet {
        wallet: wallet_handle(&eng, sanitized, network).await?,
    })
}
