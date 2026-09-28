// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Wallet lifecycle — `create_wallet`, `open_wallet`, `close_wallet`,
//! `restore_wallet` — under the wallet contract's names and result shapes.
//! A feature module (rule 27) over `EngineSession`, which does the opening.
//!
//! Every call that leaves a wallet open returns the contract's
//! `WalletHandle` and nothing else: the address is read from
//! `get_primary_address`, its one source. `restore_height_hint` follows
//! wallet-rpc — omitted on create, on a restore from genesis, and on a
//! cache-hit open; present for a higher restore floor and for an open that
//! rebuilt the ledger (`OpenedEngine::Restored`, including a zero floor).
//! `create_wallet` adds the backup exactly once, in the network's encoding
//! — `mnemonic` on mainnet/stagenet, `raw_seed_hex` on testnet — and the
//! session keeps no copy of it.

use serde::Serialize;
use shekyl_engine_core::Capability;
use tauri::State;

use crate::engine_session;
use crate::state::{AppState, NetworkType};
use crate::validate;
use crate::wallet_name;

/// The contract's `WalletHandle`: returned by every lifecycle call that
/// leaves a wallet open. Informational, not a bearer token.
#[derive(Debug, Clone, Serialize)]
pub struct WalletHandle {
    /// Wallet file stem within the wallet directory.
    pub name: String,
    /// `CapabilityMode` — the envelope's own capability, never inferred.
    pub capability: &'static str,
    /// `MAINNET | TESTNET | STAGENET`.
    pub network: &'static str,
    /// Present when a restore used a floor above genesis, or when open rebuilt
    /// the ledger. Omitted on create, on a genesis restore, and on a cache-hit open.
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

/// The handle for a wallet the session just opened. Infallible: the facts
/// were taken from the engine before it was published as open.
fn wallet_handle(
    name: String,
    network: NetworkType,
    facts: &engine_session::WalletFacts,
) -> WalletHandle {
    WalletHandle {
        name,
        capability: capability_str(facts.capability),
        network: network_str(network),
        restore_height_hint: facts.restore_height_hint,
    }
}

/// `restore_wallet`'s floor, as the keys file stores it (`u32`).
///
/// Omitted and `0` both scan from genesis. A height above `u32::MAX` is
/// refused, matching wallet-rpc, so the file never stores a truncated floor.
fn restore_floor(restore_height: Option<u64>) -> Result<u32, String> {
    match restore_height {
        None => Ok(0),
        Some(height) => u32::try_from(height).map_err(|_| {
            "Restore height is outside the range the wallet file can store".to_owned()
        }),
    }
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
    let created = eng
        .create(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    let wallet = wallet_handle(sanitized, network, &created.facts);
    let (mnemonic, raw_seed_hex) = match created.backup {
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
    let facts = eng
        .open(&wallet_dir, &sanitized, &password, network, &daemon)
        .await?;
    *state.wallet_open.write().await = true;
    *state.wallet_name.write().await = Some(sanitized.clone());
    Ok(OpenedWallet {
        wallet: wallet_handle(sanitized, network, &facts),
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

    let height = restore_floor(restore_height)?;

    let wallet_dir = state.wallet_dir.read().await.clone();
    wallet_name::ensure_dir_exists(&wallet_dir)?;

    let daemon = state.daemon_http_base().await;
    let mut eng = state.engine.lock().await;
    let facts = eng
        .restore_from_backup(
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
        wallet: wallet_handle(sanitized, network, &facts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle_from(facts: engine_session::WalletFacts) -> WalletHandle {
        wallet_handle("alice".into(), NetworkType::Testnet, &facts)
    }

    #[test]
    fn handle_spells_the_contract_and_reports_the_hint_only_when_wallet_rpc_does() {
        use engine_session::{OpenedLedger, WalletFacts};

        let created =
            serde_json::to_value(handle_from(WalletFacts::created(Capability::Full))).unwrap();
        assert_eq!(
            created,
            serde_json::json!({ "name": "alice", "capability": "FULL", "network": "TESTNET" })
        );
        assert!(created.get("restore_height_hint").is_none());

        let genesis =
            serde_json::to_value(handle_from(WalletFacts::restored(Capability::Full, 0))).unwrap();
        assert!(genesis.get("restore_height_hint").is_none());
        let restored =
            serde_json::to_value(handle_from(WalletFacts::restored(Capability::Full, 1200)))
                .unwrap();
        assert_eq!(restored["restore_height_hint"], 1200);

        // A later unlock of that same wallet loaded the state file: the hint
        // is omitted, even though the keys file still stores 1200.
        let loaded = serde_json::to_value(handle_from(WalletFacts::opened(
            Capability::Full,
            OpenedLedger::Loaded,
        )))
        .unwrap();
        assert!(loaded.get("restore_height_hint").is_none());

        // Rebuilding after the state file is lost reports the floor, including zero.
        let rebuilt_genesis = serde_json::to_value(handle_from(WalletFacts::opened(
            Capability::Full,
            OpenedLedger::Rebuilt { from_height: 0 },
        )))
        .unwrap();
        assert_eq!(rebuilt_genesis["restore_height_hint"], 0);
        let rebuilt = serde_json::to_value(handle_from(WalletFacts::opened(
            Capability::Full,
            OpenedLedger::Rebuilt { from_height: 1200 },
        )))
        .unwrap();
        assert_eq!(rebuilt["restore_height_hint"], 1200);

        assert_eq!(network_str(NetworkType::Mainnet), "MAINNET");
        assert_eq!(network_str(NetworkType::Stagenet), "STAGENET");
    }

    #[test]
    fn restore_floor_rejects_a_height_the_keys_file_cannot_store() {
        assert_eq!(restore_floor(None).unwrap(), 0);
        assert_eq!(restore_floor(Some(0)).unwrap(), 0);
        assert_eq!(restore_floor(Some(1200)).unwrap(), 1200);
        assert!(restore_floor(Some(u64::from(u32::MAX) + 1)).is_err());
    }

    #[test]
    fn created_wallet_carries_exactly_one_backup_encoding() {
        let wallet = handle_from(engine_session::WalletFacts::created(Capability::Full));
        let mainnet = serde_json::to_value(CreatedWallet {
            wallet: wallet.clone(),
            mnemonic: Some("word ".repeat(24).trim().into()),
            raw_seed_hex: None,
        })
        .unwrap();
        assert!(mainnet.get("mnemonic").is_some() && mainnet.get("raw_seed_hex").is_none());
        let testnet = serde_json::to_value(CreatedWallet {
            wallet: wallet.clone(),
            mnemonic: None,
            raw_seed_hex: Some("ab".repeat(32)),
        })
        .unwrap();
        assert!(testnet.get("mnemonic").is_none() && testnet.get("raw_seed_hex").is_some());
        let opened = serde_json::to_value(OpenedWallet { wallet }).unwrap();
        assert_eq!(opened["wallet"]["name"], "alice");
        assert!(
            opened.get("address").is_none(),
            "the address has one source: get_primary_address"
        );
    }
}
