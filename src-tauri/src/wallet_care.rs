// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Password change, refresh, and rescan.
//!
//! Refresh and rescan share counters and are still different results.
//! Only a refresh can report the height a reorg rewound to. A rescan
//! rebuilds from the wallet's scan floor, so it has no fork to name.

use serde::Serialize;
use shekyl_engine_core::{Credentials, Engine, RefreshOptions};
use shekyl_wallet_contract::error::WalletRpcError;
use tauri::State;
use zeroize::Zeroizing;

use crate::contract_error::{count_as_u64, open_engine, ContractError};
use crate::engine_session::SharedEngine;
use crate::state::AppState;
use crate::validate;

/// `refresh`. `reorg_fork_height` is present only when this refresh rewound.
#[derive(Debug, Serialize)]
pub struct RefreshOut {
    pub blocks_processed: u64,
    pub transfers_detected: u64,
    pub synced_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reorg_fork_height: Option<u64>,
}

/// `rescan_blockchain`. No reorg field: a rescan is not a rewind.
#[derive(Debug, Serialize)]
pub struct RescanOut {
    pub blocks_processed: u64,
    pub transfers_detected: u64,
    pub synced_height: u64,
}

#[tauri::command]
pub async fn refresh(state: State<'_, AppState>) -> Result<RefreshOut, ContractError> {
    let shared = open_engine(&state).await?;
    let handle = Engine::start_refresh(shared.clone(), RefreshOptions::default())
        .await
        .map_err(ContractError::from_engine)?;
    let summary = handle.join().await.map_err(ContractError::from_engine)?;
    Ok(RefreshOut {
        blocks_processed: summary.blocks_processed,
        transfers_detected: count_as_u64(summary.transfers_detected),
        synced_height: synced_height(&shared).await,
        reorg_fork_height: summary
            .reorg
            .as_ref()
            .map(|event| event.fork_height.to_raw()),
    })
}

#[tauri::command]
pub async fn rescan_blockchain(state: State<'_, AppState>) -> Result<RescanOut, ContractError> {
    let shared = open_engine(&state).await?;
    let handle = Engine::start_rescan(shared.clone(), RefreshOptions::default())
        .await
        .map_err(ContractError::from_engine)?;
    let summary = handle
        .join()
        .await
        .map_err(WalletRpcError::from_rescan_scan_failure)
        .map_err(ContractError::from_rpc)?;
    Ok(RescanOut {
        blocks_processed: summary.blocks_processed,
        transfers_detected: count_as_u64(summary.transfers_detected),
        synced_height: synced_height(&shared).await,
    })
}

#[tauri::command]
pub async fn change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), ContractError> {
    accept_passwords(&old_password, &new_password)?;
    let shared = open_engine(&state).await?;
    let old = Zeroizing::new(old_password.into_bytes());
    let new = Zeroizing::new(new_password.into_bytes());
    let old_creds = Credentials::password_only(old.as_slice());
    let new_creds = Credentials::password_only(new.as_slice());
    let mut engine = shared.write().await;
    tokio::task::block_in_place(|| engine.change_password(&old_creds, &new_creds, None))
        .map_err(ContractError::from_engine)?;
    Ok(())
}

/// The same command-edge screen create and open use: length cap and no NUL.
/// A short password is still a password; creation's eight-character floor
/// is the page's, so an existing shorter one can still be changed.
fn accept_passwords(old_password: &str, new_password: &str) -> Result<(), ContractError> {
    validate::validate_password(old_password).map_err(ContractError::invalid)?;
    validate::validate_password(new_password).map_err(ContractError::invalid)?;
    Ok(())
}

async fn synced_height(shared: &SharedEngine) -> u64 {
    shared.read().await.ledger().ledger.height().to_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_byte_in_either_password_is_invalid_params() {
        let err = accept_passwords("current", "next\0").expect_err("null");
        assert_eq!(err.code, "INVALID_PARAMS");
        assert_eq!(
            err.message,
            "invalid params: Password must not contain null bytes"
        );
        assert!(accept_passwords("old\0", "nextpassword").is_err());
    }

    #[test]
    fn refresh_omits_the_fork_when_the_scan_was_linear() {
        let json = serde_json::to_value(RefreshOut {
            blocks_processed: 0,
            transfers_detected: 0,
            synced_height: 4,
            reorg_fork_height: None,
        })
        .unwrap();
        assert!(json.get("reorg_fork_height").is_none());
    }

    #[test]
    fn rescan_has_no_fork_field_to_serialize() {
        let json = serde_json::to_value(RescanOut {
            blocks_processed: 1,
            transfers_detected: 2,
            synced_height: 3,
        })
        .unwrap();
        assert!(json.get("reorg_fork_height").is_none());
        assert_eq!(json["synced_height"], 3);
    }
}
