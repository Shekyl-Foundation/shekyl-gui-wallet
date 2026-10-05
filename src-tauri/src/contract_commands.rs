// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Wallet-contract methods the desktop wallet did not yet expose:
//! proofs, message signing, password change, refresh, rescan, notes,
//! abandon, and transfer lookup.
//!
//! Each command calls the same engine entry the wallet-rpc handler calls.

use serde::Serialize;
use shekyl_address::ShekylAddress;
use shekyl_crypto_pq::message_signing::MessageSigError;
use shekyl_engine_core::engine::message_signing::{
    self as engine_signing, SignMessageError, VerifyMessageError,
};
use shekyl_engine_core::engine::proofs::{self, ProofsError};
use shekyl_engine_core::{Credentials, Engine, RefreshOptions};
use shekyl_engine_state::check_tx_note_len;
use shekyl_types::TxHash;
use shekyl_units::{AtomicUnits, AtomicUnitsString};
use shekyl_wallet_contract::error::WalletRpcError;
use tauri::State;
use zeroize::Zeroizing;

use crate::engine_daemon;
use crate::engine_session::map_network;
use crate::send;
use crate::staking_actions::ActionError;
use crate::state::AppState;

fn contract(err: WalletRpcError) -> ActionError {
    ActionError::from_contract(err)
}

fn hex32(s: &str) -> Result<[u8; 32], ActionError> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(contract(WalletRpcError::InvalidParams(
            "txid must be 64 lowercase hex characters".into(),
        )));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| {
            contract(WalletRpcError::InvalidParams(
                "txid must be 64 lowercase hex characters".into(),
            ))
        })?;
    }
    Ok(out)
}

fn proof_address(
    s: &str,
    network: shekyl_engine_core::Network,
) -> Result<ShekylAddress, ActionError> {
    let address = ShekylAddress::decode_for_network(s, network)
        .map_err(|_| contract(WalletRpcError::InvalidRecipient))?;
    if !address.has_pqc_segment() {
        return Err(contract(WalletRpcError::InvalidRecipient));
    }
    Ok(address)
}

fn map_proofs(e: ProofsError) -> ActionError {
    let err = match e {
        ProofsError::Malformed(_) => WalletRpcError::ProofMalformed,
        ProofsError::TxSecretUnavailable => WalletRpcError::ProofTxSecretUnavailable,
        ProofsError::NoProvableOutputs(_) => WalletRpcError::ProofNoProvableOutputs,
        ProofsError::TxNotFound(_) => WalletRpcError::ProofTxNotFound,
        ProofsError::TxUnconfirmed(_) => WalletRpcError::ProofTxUnconfirmed,
        ProofsError::DaemonSyncing => WalletRpcError::ProofDaemonSyncing,
        ProofsError::InvalidRecipient => WalletRpcError::InvalidRecipient,
        ProofsError::AmountOverflow => {
            WalletRpcError::InternalError("proof amount sum overflow".into())
        }
        ProofsError::Daemon(e) => shekyl_wallet_contract::error::from_daemon_rpc_error(&e),
        ProofsError::Key(_) => WalletRpcError::InternalError("proof key-engine failure".into()),
        ProofsError::Generate(_) => {
            WalletRpcError::InternalError("proof generation failure".into())
        }
        ProofsError::Encoding(_) => WalletRpcError::InternalError("proof encoding failure".into()),
    };
    contract(err)
}

#[derive(Serialize)]
pub struct TxProofOut {
    pub proof: String,
    pub direction: String,
}

#[derive(Serialize)]
pub struct ReserveProofOut {
    pub proof: String,
    pub total: AtomicUnitsString,
    pub output_count: u64,
}

#[derive(Serialize)]
pub struct TxCheckOut {
    pub valid: bool,
    pub direction: Option<String>,
    pub received: Option<AtomicUnitsString>,
    pub in_pool: Option<bool>,
    pub confirmations: Option<u64>,
}

#[derive(Serialize)]
pub struct ReserveCheckOut {
    pub valid: bool,
    pub total: Option<AtomicUnitsString>,
    pub spent: Option<AtomicUnitsString>,
}

#[tauri::command]
pub async fn get_tx_proof(
    state: State<'_, AppState>,
    txid: String,
    address: String,
    message: Option<String>,
) -> Result<TxProofOut, ActionError> {
    let txid = hex32(&txid)?;
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    let decoded = proof_address(&address, engine.network())?;
    let generated = engine
        .get_tx_proof(
            TxHash::from_bytes(txid),
            &decoded,
            message.as_deref().unwrap_or(""),
        )
        .await
        .map_err(map_proofs)?;
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
) -> Result<ReserveProofOut, ActionError> {
    let amount = amount.map(AtomicUnitsString::to_atomic_units);
    if amount == Some(AtomicUnits::ZERO) {
        return Err(contract(WalletRpcError::InvalidParams(
            "amount must be positive".into(),
        )));
    }
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    let generated = engine
        .get_reserve_proof(amount, message.as_deref().unwrap_or(""))
        .await
        .map_err(map_proofs)?;
    Ok(ReserveProofOut {
        proof: generated.proof,
        total: generated.total.into(),
        output_count: generated.output_count as u64,
    })
}

#[tauri::command]
pub async fn check_tx_proof(
    state: State<'_, AppState>,
    txid: String,
    address: String,
    proof: String,
    message: Option<String>,
) -> Result<TxCheckOut, ActionError> {
    let txid = hex32(&txid)?;
    let network = map_network(*state.network.read().await);
    let decoded = proof_address(&address, network)?;
    let daemon = engine_daemon::make_daemon(&state.daemon_url().await, network)
        .await
        .map_err(|e| contract(WalletRpcError::InternalError(e)))?;
    let checked = proofs::check_tx_proof(
        &daemon,
        txid,
        &decoded,
        message.as_deref().unwrap_or(""),
        &proof,
    )
    .await
    .map_err(map_proofs)?;
    Ok(match checked {
        proofs::CheckedTxProof::Invalid => TxCheckOut {
            valid: false,
            direction: None,
            received: None,
            in_pool: None,
            confirmations: None,
        },
        proofs::CheckedTxProof::Valid {
            direction,
            received,
            in_pool,
            confirmations,
            ..
        } => TxCheckOut {
            valid: true,
            direction: Some(direction.as_contract_str().to_owned()),
            received: Some(received.into()),
            in_pool: Some(in_pool),
            confirmations: Some(confirmations),
        },
    })
}

#[tauri::command]
pub async fn check_reserve_proof(
    state: State<'_, AppState>,
    address: String,
    proof: String,
    message: Option<String>,
) -> Result<ReserveCheckOut, ActionError> {
    let network = map_network(*state.network.read().await);
    let decoded = proof_address(&address, network)?;
    let daemon = engine_daemon::make_daemon(&state.daemon_url().await, network)
        .await
        .map_err(|e| contract(WalletRpcError::InternalError(e)))?;
    let checked =
        proofs::check_reserve_proof(&daemon, &decoded, message.as_deref().unwrap_or(""), &proof)
            .await
            .map_err(map_proofs)?;
    Ok(match checked {
        proofs::CheckedReserveProof::Invalid => ReserveCheckOut {
            valid: false,
            total: None,
            spent: None,
        },
        proofs::CheckedReserveProof::Valid { total, spent, .. } => ReserveCheckOut {
            valid: true,
            total: Some(total.into()),
            spent: Some(spent.into()),
        },
    })
}

fn map_sig(e: &MessageSigError) -> WalletRpcError {
    match e {
        MessageSigError::Malformed(detail) => {
            WalletRpcError::InvalidParams(format!("malformed signature string: {detail}"))
        }
        MessageSigError::UnsupportedScheme(scheme) => {
            WalletRpcError::MessageSigUnsupportedScheme { scheme: *scheme }
        }
        MessageSigError::Corrupted => WalletRpcError::MessageSigCorrupted,
        MessageSigError::VerifyFailed => WalletRpcError::MessageSigVerifyFailed,
        MessageSigError::InvalidKey => {
            WalletRpcError::InternalError("message-signing key material invalid".into())
        }
        MessageSigError::Rng => WalletRpcError::InternalError(
            "the system random number generator failed — try again".into(),
        ),
    }
}

#[derive(Serialize)]
pub struct SignatureOut {
    pub signature: String,
}

#[derive(Serialize)]
pub struct VerifyOut {
    pub valid: bool,
}

#[tauri::command]
pub async fn sign_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<SignatureOut, ActionError> {
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    let signature = engine.sign_message(message.as_bytes()).await.map_err(|e| {
        let err = match e {
            SignMessageError::WalletSessionEnded => WalletRpcError::WalletSessionEnded,
            SignMessageError::Key(_) => {
                WalletRpcError::InternalError("sign_message key-engine failure".into())
            }
            SignMessageError::Crypto(inner) => map_sig(&inner),
            SignMessageError::Internal(_) => {
                WalletRpcError::InternalError("sign_message internal failure".into())
            }
        };
        contract(err)
    })?;
    Ok(SignatureOut { signature })
}

#[tauri::command]
pub async fn verify_message(
    state: State<'_, AppState>,
    address: String,
    message: String,
    signature: String,
) -> Result<VerifyOut, ActionError> {
    let network = map_network(*state.network.read().await);
    let result = tokio::task::spawn_blocking(move || {
        engine_signing::verify_message(network, &address, message.as_bytes(), &signature)
    })
    .await
    .map_err(|e| contract(WalletRpcError::InternalError(format!("verify failed: {e}"))))?;
    match result {
        Ok(_) => Ok(VerifyOut { valid: true }),
        Err(VerifyMessageError::Crypto(MessageSigError::VerifyFailed)) => {
            Ok(VerifyOut { valid: false })
        }
        Err(VerifyMessageError::InvalidAddress | VerifyMessageError::ClassicalOnly) => {
            Err(contract(WalletRpcError::InvalidParams(
                "the address is not a full Shekyl address".into(),
            )))
        }
        Err(VerifyMessageError::Crypto(inner)) => Err(contract(map_sig(&inner))),
    }
}

#[derive(Serialize)]
pub struct ScanOut {
    pub blocks_processed: u64,
    pub transfers_detected: u64,
    pub synced_height: u64,
}

#[tauri::command]
pub async fn refresh(state: State<'_, AppState>) -> Result<ScanOut, ActionError> {
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let handle = Engine::start_refresh(shared.clone(), RefreshOptions::default())
        .await
        .map_err(|e| contract(e.into()))?;
    let summary = handle.join().await.map_err(|e| contract(e.into()))?;
    let height = shared.read().await.ledger().ledger.height().to_raw();
    Ok(ScanOut {
        blocks_processed: summary.blocks_processed,
        transfers_detected: summary.transfers_detected as u64,
        synced_height: height,
    })
}

#[tauri::command]
pub async fn rescan_blockchain(state: State<'_, AppState>) -> Result<ScanOut, ActionError> {
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let handle = Engine::start_rescan(shared.clone(), RefreshOptions::default())
        .await
        .map_err(|e| contract(e.into()))?;
    let summary = handle
        .join()
        .await
        .map_err(WalletRpcError::from_rescan_scan_failure)
        .map_err(contract)?;
    let height = shared.read().await.ledger().ledger.height().to_raw();
    Ok(ScanOut {
        blocks_processed: summary.blocks_processed,
        transfers_detected: summary.transfers_detected as u64,
        synced_height: height,
    })
}

#[tauri::command]
pub async fn change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), ActionError> {
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let old = Zeroizing::new(old_password.into_bytes());
    let new = Zeroizing::new(new_password.into_bytes());
    let old_creds = Credentials::password_only(old.as_slice());
    let new_creds = Credentials::password_only(new.as_slice());
    let mut engine = shared.write().await;
    tokio::task::block_in_place(|| engine.change_password(&old_creds, &new_creds, None))
        .map_err(|e| contract(e.into()))?;
    Ok(())
}

#[derive(Serialize)]
pub struct NoteOut {
    pub tx_hash: String,
    pub note: Option<String>,
}

#[tauri::command]
pub async fn set_tx_note(
    state: State<'_, AppState>,
    tx_hash: String,
    note: String,
) -> Result<NoteOut, ActionError> {
    let txid = TxHash::from_bytes(hex32(&tx_hash)?);
    check_tx_note_len(&note).map_err(|e| contract(e.into()))?;
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    let stored = engine
        .set_tx_note(txid, note)
        .map_err(|e| contract(e.into()))?;
    Ok(NoteOut {
        tx_hash,
        note: stored,
    })
}

#[tauri::command]
pub async fn get_tx_note(
    state: State<'_, AppState>,
    tx_hash: String,
) -> Result<NoteOut, ActionError> {
    let txid = TxHash::from_bytes(hex32(&tx_hash)?);
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    let note = engine.tx_note(txid);
    Ok(NoteOut { tx_hash, note })
}

#[derive(Serialize)]
pub struct AbandonOut {
    pub state: &'static str,
}

#[tauri::command]
pub async fn abandon_tx(
    state: State<'_, AppState>,
    tx_hash: String,
) -> Result<AbandonOut, ActionError> {
    let txid = TxHash::from_bytes(hex32(&tx_hash)?);
    let shared = send::shared_engine(&state)
        .await
        .map_err(|_| ActionError::closed())?;
    let engine = shared.read().await;
    engine
        .abandon_tx_persisted(txid)
        .map_err(|e| contract(e.into()))?;
    Ok(AbandonOut { state: "ABANDONED" })
}

#[tauri::command]
pub async fn get_transfer_by_id(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::transfer_history::TransferRow, ActionError> {
    let mut eng = state.engine.lock().await;
    let rows = eng
        .list_transfers()
        .await
        .map_err(|e| contract(WalletRpcError::InternalError(e)))?;
    rows.into_iter()
        .find(|row| row.id == id || row.tx_hash == id)
        .ok_or_else(|| contract(WalletRpcError::InvalidParams("unknown transfer".into())))
}
