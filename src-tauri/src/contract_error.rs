// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The desktop wallet's view of a contract error.
//!
//! Every contract command returns this one shape: the contract's code name
//! and the sentence [`shekyl_wallet_contract::error::WalletRpcError`] already
//! chose. Surfaces do not keep a second map.

use serde::Serialize;
use shekyl_types::TxHash;
use shekyl_wallet_contract::canonical_hex::{self, HEX32_BYTES};
use shekyl_wallet_contract::error::WalletRpcError;

use crate::engine_session::SharedEngine;
use crate::send;
use crate::state::AppState;

/// A contract failure, as the page reads it (`code` + `message`).
#[derive(Debug, Serialize)]
pub struct ContractError {
    pub code: &'static str,
    pub message: String,
}

impl ContractError {
    pub(crate) fn from_rpc(err: WalletRpcError) -> Self {
        Self {
            code: err.code().name(),
            message: err.message(),
        }
    }

    pub(crate) fn from_engine(err: impl Into<WalletRpcError>) -> Self {
        Self::from_rpc(err.into())
    }

    pub(crate) fn closed() -> Self {
        Self::from_rpc(WalletRpcError::WalletNotOpen)
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::from_rpc(WalletRpcError::InvalidParams(message.into()))
    }
}

/// The open engine, or [`ContractError::closed`].
pub(crate) async fn open_engine(state: &AppState) -> Result<SharedEngine, ContractError> {
    send::shared_engine(state)
        .await
        .map_err(|_| ContractError::closed())
}

/// A producer count onto this edge.
///
/// The engine counts in `u64` or `usize`. A count that does not fit is
/// reported as [`u64::MAX`] rather than truncated with `as`.
pub(crate) fn count_as_u64(count: impl TryInto<u64>) -> u64 {
    count.try_into().unwrap_or(u64::MAX)
}

/// A canonical tx hash, or invalid params naming `field`.
pub(crate) fn require_tx_hash(value: &str, field: &str) -> Result<TxHash, ContractError> {
    canonical_hex::parse_lowercase_hex32(value)
        .map(TxHash::from_bytes)
        .ok_or_else(|| ContractError::invalid(canonical_hex::invalid_hex32_message(field)))
}

/// Proofs name the field `txid`. Notes and abandon name it `tx_hash`.
pub(crate) const TXID_FIELD: &str = "txid";
pub(crate) const TX_HASH_FIELD: &str = "tx_hash";

const _: () = assert!(HEX32_BYTES == 32);
