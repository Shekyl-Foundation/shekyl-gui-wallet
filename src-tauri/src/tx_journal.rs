// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Notes and abandon: the user's own record of a transaction.
//!
//! A cleared note is omitted, not written as JSON null. An unknown tx
//! hash on read is an empty note, not an error — a note does not claim
//! the transaction exists. Abandon answers with the history state's
//! `ABANDONED`, the same word the transactions list uses.

use serde::Serialize;
use shekyl_engine_state::check_tx_note_len;
use tauri::State;

use crate::contract_error::{self, open_engine, ContractError, TX_HASH_FIELD};
use crate::state::AppState;
use crate::transfer_history::TransferState;

#[derive(Debug, Serialize)]
pub struct NoteOut {
    pub tx_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AbandonOut {
    pub state: TransferState,
}

#[tauri::command]
pub async fn set_tx_note(
    state: State<'_, AppState>,
    tx_hash: String,
    note: String,
) -> Result<NoteOut, ContractError> {
    let txid = contract_error::require_tx_hash(&tx_hash, TX_HASH_FIELD)?;
    check_tx_note_len(&note).map_err(ContractError::from_engine)?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let stored = engine
        .set_tx_note(txid, note)
        .map_err(ContractError::from_engine)?;
    Ok(NoteOut {
        tx_hash: txid.to_string(),
        note: stored,
    })
}

#[tauri::command]
pub async fn get_tx_note(
    state: State<'_, AppState>,
    tx_hash: String,
) -> Result<NoteOut, ContractError> {
    let txid = contract_error::require_tx_hash(&tx_hash, TX_HASH_FIELD)?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    Ok(NoteOut {
        tx_hash: txid.to_string(),
        note: engine.tx_note(txid),
    })
}

#[tauri::command]
pub async fn abandon_tx(
    state: State<'_, AppState>,
    tx_hash: String,
) -> Result<AbandonOut, ContractError> {
    let txid = contract_error::require_tx_hash(&tx_hash, TX_HASH_FIELD)?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    engine
        .abandon_tx_persisted(txid)
        .map_err(ContractError::from_engine)?;
    Ok(AbandonOut {
        state: TransferState::Abandoned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cleared_note_omits_the_field() {
        let json = serde_json::to_value(NoteOut {
            tx_hash: "ab".repeat(32),
            note: None,
        })
        .unwrap();
        assert!(json.get("note").is_none());
    }

    #[test]
    fn abandon_answers_in_the_history_vocabulary() {
        let json = serde_json::to_value(AbandonOut {
            state: TransferState::Abandoned,
        })
        .unwrap();
        assert_eq!(json["state"], "ABANDONED");
    }
}
