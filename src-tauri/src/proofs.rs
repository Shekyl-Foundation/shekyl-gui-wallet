// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Payment proofs and reserve proofs.
//!
//! The four contract methods. A proof that parses but does not check out
//! is `valid: false` — an answer, not an error. Framing failures, a
//! missing tx secret, and a syncing daemon are errors, mapped by
//! `shekyl-wallet-contract` so this wallet and the RPC server say the
//! same thing. Check is wallet-less: it dials the daemon directly.

use serde::Serialize;
use shekyl_address::ShekylAddress;
use shekyl_engine_core::engine::proofs::{self, CheckedReserveProof, CheckedTxProof};
use shekyl_engine_core::Network;
use shekyl_units::{AtomicUnits, AtomicUnitsString};
use shekyl_wallet_contract::error::WalletRpcError;
use tauri::State;

use crate::contract_error::{self, count_as_u64, open_engine, ContractError, TXID_FIELD};
use crate::engine_daemon;
use crate::engine_session::map_network;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct TxProofOut {
    pub proof: String,
    pub direction: String,
}

#[derive(Debug, Serialize)]
pub struct ReserveProofOut {
    pub proof: String,
    pub total: AtomicUnitsString,
    pub output_count: u64,
}

#[derive(Debug, Serialize)]
pub struct TxProofOutputOut {
    pub output_index: u64,
    pub amount: AtomicUnitsString,
}

/// `check_tx_proof`. Fields other than `valid` are present only when the
/// proof checks out.
#[derive(Debug, Serialize)]
pub struct TxCheckOut {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<AtomicUnitsString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<TxProofOutputOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_pool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u64>,
}

/// `check_reserve_proof`. The live reserve is `total - spent`.
#[derive(Debug, Serialize)]
pub struct ReserveCheckOut {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<AtomicUnitsString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spent: Option<AtomicUnitsString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_count: Option<u64>,
}

#[tauri::command]
pub async fn get_tx_proof(
    state: State<'_, AppState>,
    txid: String,
    address: String,
    message: Option<String>,
) -> Result<TxProofOut, ContractError> {
    let txid = contract_error::require_tx_hash(&txid, TXID_FIELD)?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let decoded = proof_address(&address, engine.network())?;
    let generated = engine
        .get_tx_proof(txid, &decoded, message.as_deref().unwrap_or(""))
        .await
        .map_err(ContractError::from_engine)?;
    Ok(TxProofOut {
        proof: generated.proof,
        direction: generated.direction.as_contract_str().to_owned(),
    })
}

#[tauri::command]
pub async fn get_reserve_proof(
    state: State<'_, AppState>,
    amount: Option<AtomicUnitsString>,
    message: Option<String>,
) -> Result<ReserveProofOut, ContractError> {
    let amount = amount.map(AtomicUnitsString::to_atomic_units);
    if amount == Some(AtomicUnits::ZERO) {
        return Err(ContractError::invalid("amount must be positive"));
    }
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let generated = engine
        .get_reserve_proof(amount, message.as_deref().unwrap_or(""))
        .await
        .map_err(ContractError::from_engine)?;
    Ok(ReserveProofOut {
        proof: generated.proof,
        total: generated.total.into(),
        output_count: count_as_u64(generated.output_count),
    })
}

#[tauri::command]
pub async fn check_tx_proof(
    state: State<'_, AppState>,
    txid: String,
    address: String,
    proof: String,
    message: Option<String>,
) -> Result<TxCheckOut, ContractError> {
    let txid = contract_error::require_tx_hash(&txid, TXID_FIELD)?;
    let network = map_network(*state.network.read().await);
    let decoded = proof_address(&address, network)?;
    let daemon = proof_daemon(&state, network).await?;
    let checked = proofs::check_tx_proof(
        &daemon,
        txid.to_bytes(),
        &decoded,
        message.as_deref().unwrap_or(""),
        &proof,
    )
    .await
    .map_err(ContractError::from_engine)?;
    Ok(project_tx_check(checked))
}

#[tauri::command]
pub async fn check_reserve_proof(
    state: State<'_, AppState>,
    address: String,
    proof: String,
    message: Option<String>,
) -> Result<ReserveCheckOut, ContractError> {
    let network = map_network(*state.network.read().await);
    let decoded = proof_address(&address, network)?;
    let daemon = proof_daemon(&state, network).await?;
    let checked =
        proofs::check_reserve_proof(&daemon, &decoded, message.as_deref().unwrap_or(""), &proof)
            .await
            .map_err(ContractError::from_engine)?;
    Ok(project_reserve_check(checked))
}

fn project_tx_check(checked: CheckedTxProof) -> TxCheckOut {
    match checked {
        CheckedTxProof::Invalid => TxCheckOut {
            valid: false,
            direction: None,
            received: None,
            outputs: None,
            in_pool: None,
            confirmations: None,
        },
        CheckedTxProof::Valid {
            direction,
            received,
            outputs,
            in_pool,
            confirmations,
        } => TxCheckOut {
            valid: true,
            direction: Some(direction.as_contract_str().to_owned()),
            received: Some(received.into()),
            outputs: Some(
                outputs
                    .iter()
                    .map(|output| TxProofOutputOut {
                        output_index: output.output_index,
                        amount: output.amount.into(),
                    })
                    .collect(),
            ),
            in_pool: Some(in_pool),
            confirmations: Some(confirmations),
        },
    }
}

fn project_reserve_check(checked: CheckedReserveProof) -> ReserveCheckOut {
    match checked {
        CheckedReserveProof::Invalid => ReserveCheckOut {
            valid: false,
            total: None,
            spent: None,
            output_count: None,
        },
        CheckedReserveProof::Valid {
            total,
            spent,
            output_count,
        } => ReserveCheckOut {
            valid: true,
            total: Some(total.into()),
            spent: Some(spent.into()),
            output_count: Some(count_as_u64(output_count)),
        },
    }
}

/// Full hybrid address. The classical-only form can never verify, so it
/// is refused here rather than reported as a confusing `valid: false`.
fn proof_address(value: &str, network: Network) -> Result<ShekylAddress, ContractError> {
    let address = ShekylAddress::decode_for_network(value, network).map_err(|err| {
        tracing::warn!(detail = %err, "proof address decode failed");
        ContractError::from_rpc(WalletRpcError::InvalidRecipient)
    })?;
    if !address.has_pqc_segment() {
        tracing::warn!("proof address lacks the PQC segment");
        return Err(ContractError::from_rpc(WalletRpcError::InvalidRecipient));
    }
    Ok(address)
}

async fn proof_daemon(
    state: &AppState,
    network: Network,
) -> Result<shekyl_engine_core::DaemonClient, ContractError> {
    engine_daemon::make_daemon(&state.base_url().await, network)
        .await
        .map_err(|err| ContractError::from_rpc(WalletRpcError::InternalError(err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_units::AtomicUnits;

    #[test]
    fn an_invalid_payment_proof_omits_the_success_fields() {
        let json = serde_json::to_value(project_tx_check(CheckedTxProof::Invalid)).unwrap();
        assert_eq!(json["valid"], false);
        assert!(json.get("received").is_none());
        assert!(json.get("outputs").is_none());
        assert!(json.get("confirmations").is_none());
    }

    #[test]
    fn a_valid_reserve_proof_carries_the_output_count() {
        let json = serde_json::to_value(project_reserve_check(CheckedReserveProof::Valid {
            total: AtomicUnits::from_raw(5),
            spent: AtomicUnits::from_raw(2),
            output_count: 3,
        }))
        .unwrap();
        assert_eq!(json["valid"], true);
        assert_eq!(json["output_count"], 3);
        assert_eq!(json["spent"], "2");
    }
}
