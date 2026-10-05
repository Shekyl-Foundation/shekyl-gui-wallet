// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Sign a message with the open wallet, or check a signature.
//!
//! Verification is success-only. A mismatch, a damaged paste, and an
//! unknown scheme are three different errors (`-29800`, `-29801`,
//! `-29802`), mapped in `shekyl-wallet-contract`. A `verified: false`
//! payload cannot carry those remedies, so this command never returns one.

use serde::Serialize;
use shekyl_engine_core::engine::message_signing as engine_signing;
use shekyl_wallet_contract::error::WalletRpcError;
use tauri::State;

use crate::contract_error::{open_engine, ContractError};
use crate::engine_session::map_network;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SignatureOut {
    pub signature: String,
}

/// Always `true`. The type cannot represent a failed check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified;

impl Serialize for Verified {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

#[derive(Debug, Serialize)]
pub struct VerifyMessageOut {
    pub verified: Verified,
}

#[tauri::command]
pub async fn sign_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<SignatureOut, ContractError> {
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let signature = engine
        .sign_message(message.as_bytes())
        .await
        .map_err(ContractError::from_engine)?;
    Ok(SignatureOut { signature })
}

#[tauri::command]
pub async fn verify_message(
    state: State<'_, AppState>,
    address: String,
    message: String,
    signature: String,
) -> Result<VerifyMessageOut, ContractError> {
    let network = map_network(*state.network.read().await);
    tokio::task::spawn_blocking(move || {
        engine_signing::verify_message(network, &address, message.as_bytes(), &signature)
    })
    .await
    .map_err(|err| {
        tracing::warn!(detail = %err, "verify_message task failed");
        ContractError::from_rpc(WalletRpcError::InternalError(
            "verify_message task failed".into(),
        ))
    })?
    .map_err(ContractError::from_engine)?;
    Ok(VerifyMessageOut { verified: Verified })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_success_serializes_only_true() {
        let json = serde_json::to_value(VerifyMessageOut { verified: Verified }).unwrap();
        assert_eq!(json, serde_json::json!({ "verified": true }));
    }
}
