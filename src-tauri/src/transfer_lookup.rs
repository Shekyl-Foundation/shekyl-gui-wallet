// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! `get_transfer_by_id`.
//!
//! The id grammar lives in `shekyl-wallet-contract`. A receive id is
//! `{tx_hash}:{output_index}`; a send id is the bare tx hash. Looking
//! those up by scanning every row for "id or tx hash" returns a receive
//! when the caller asked for the send of the same transaction. This
//! command parses the id, then reads that one side of the ledger.

use serde::Serialize;
use shekyl_types::OutputIndexInTx;
use shekyl_wallet_contract::error::WalletRpcError;
use shekyl_wallet_contract::transfer_id::{self, TransferLookupId};
use tauri::State;

use crate::contract_error::{open_engine, ContractError};
use crate::state::AppState;
use crate::transfer_history::{
    project_incoming_row, project_outgoing_row, IncomingFact, TransferRow,
};

/// The contract's `get_transfer_by_id` result.
#[derive(Debug, Serialize)]
pub struct TransferById {
    pub transfer: TransferRow,
}

#[tauri::command]
pub async fn get_transfer_by_id(
    state: State<'_, AppState>,
    id: String,
) -> Result<TransferById, ContractError> {
    let lookup = transfer_id::parse_lookup_id(&id)
        .ok_or_else(|| ContractError::invalid(transfer_id::LOOKUP_ID_GRAMMAR))?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let ledger = engine.ledger();
    let locks = ledger.spend_locks();
    let transfer = match lookup {
        TransferLookupId::Incoming {
            tx_hash,
            output_index,
        } => {
            let index = OutputIndexInTx::from_raw(output_index);
            ledger
                .ledger
                .transfers()
                .iter()
                .find(|row| row.tx_hash == tx_hash && row.internal_output_index == index)
                .map(|row| project_incoming_row(&IncomingFact::from_details(row, &locks)))
        }
        TransferLookupId::Outgoing { tx_hash } => {
            let bytes = tx_hash.to_bytes();
            match ledger.send_journal.rows.get(&bytes) {
                None => None,
                Some(record) => Some(project_outgoing_row(&bytes, record).map_err(|detail| {
                    tracing::warn!(detail, "send journal row amounts do not sum");
                    ContractError::from_rpc(WalletRpcError::InternalError(
                        "send journal row has recipient amounts that do not sum".into(),
                    ))
                })?),
            }
        }
    };
    let transfer =
        transfer.ok_or_else(|| ContractError::from_rpc(WalletRpcError::UnknownTransferId))?;
    Ok(TransferById { transfer })
}
