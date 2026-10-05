// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Staking money movement, mirroring wallet-rpc `staking_actions.rs`.
//!
//! `stake_in` returns the same reservation shape as `build_pending_tx`, so
//! the page confirms and submits through the existing send commands.
//! `drain`, `unstake`, and `collect_unstaked` fire on the call. The page
//! labels `unstake` Release. The command name stays the contract's.

use serde::Serialize;
use shekyl_engine_core::{CollectOutcome, DrainOutcome, StakeFacade, UnstakeOutcome};
use shekyl_units::AtomicUnitsString;
use shekyl_wallet_contract::error::WalletRpcError;
use tauri::State;

use crate::send::{self, BuiltPendingTx};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ActionError {
    pub code: &'static str,
    pub message: String,
}

impl ActionError {
    pub(crate) fn from_contract(err: WalletRpcError) -> Self {
        Self {
            code: err.code().name(),
            message: err.message(),
        }
    }

    pub(crate) fn closed() -> Self {
        Self::from_contract(WalletRpcError::WalletNotOpen)
    }
}

/// Receipt shared by drain and release (`DrainResult` / `UnstakeResult`).
#[derive(Debug, Serialize)]
pub struct SealedReceipt {
    pub tx_hash: String,
    pub verdict: &'static str,
    pub confirmed_height: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CollectReceipt {
    pub kind: &'static str,
    pub tx_hash: Option<String>,
    pub swept: Option<AtomicUnitsString>,
    pub remainder: Option<AtomicUnitsString>,
    pub another_pool_remains: Option<bool>,
}

async fn open_engine(state: &AppState) -> Result<crate::engine_session::SharedEngine, ActionError> {
    send::shared_engine(state)
        .await
        .map_err(|_| ActionError::closed())
}

#[tauri::command]
pub async fn stake_in(
    state: State<'_, AppState>,
    amount: AtomicUnitsString,
) -> Result<BuiltPendingTx, ActionError> {
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let pending = engine
        .stake()
        .stake_in(amount.to_atomic_units())
        .await
        .map_err(|e| ActionError::from_contract(e.into()))?;
    Ok(BuiltPendingTx {
        pending_tx_id: pending.id.raw().to_string(),
        fee: pending.fee_atomic_units.into(),
        content_gen: pending.content_gen,
    })
}

#[tauri::command]
pub async fn drain(
    state: State<'_, AppState>,
    amount: AtomicUnitsString,
) -> Result<SealedReceipt, ActionError> {
    let payment = amount.to_atomic_units();
    if payment.is_zero() {
        return Err(ActionError::from_contract(WalletRpcError::InvalidParams(
            "the amount must be greater than zero".into(),
        )));
    }
    let shared = open_engine(&state).await?;
    let outcome = StakeFacade::drain_to_principal(shared, payment)
        .await
        .map_err(|e| ActionError::from_contract(e.into()))?;
    Ok(seal_drain(&outcome))
}

fn seal_drain(outcome: &DrainOutcome) -> SealedReceipt {
    match outcome {
        DrainOutcome::Broadcast { tx_hash } => SealedReceipt {
            tx_hash: tx_hash.to_string(),
            verdict: "BROADCAST",
            confirmed_height: None,
        },
        DrainOutcome::AlreadyInChain { tx_hash, height } => SealedReceipt {
            tx_hash: tx_hash.to_string(),
            verdict: "ALREADY_IN_CHAIN",
            confirmed_height: Some(*height),
        },
    }
}

#[tauri::command]
pub async fn unstake(state: State<'_, AppState>) -> Result<SealedReceipt, ActionError> {
    let shared = open_engine(&state).await?;
    let daemon = state.daemon_url().await;
    let outcome = StakeFacade::unstake(shared, &daemon)
        .await
        .map_err(|e| ActionError::from_contract(e.into()))?;
    Ok(match outcome {
        UnstakeOutcome::Broadcast { tx_hash } => SealedReceipt {
            tx_hash: tx_hash.to_string(),
            verdict: "BROADCAST",
            confirmed_height: None,
        },
        UnstakeOutcome::AlreadyInChain { tx_hash, height } => SealedReceipt {
            tx_hash: tx_hash.to_string(),
            verdict: "ALREADY_IN_CHAIN",
            confirmed_height: Some(height),
        },
    })
}

#[tauri::command]
pub async fn collect_unstaked(state: State<'_, AppState>) -> Result<CollectReceipt, ActionError> {
    let shared = open_engine(&state).await?;
    let outcome = StakeFacade::collect_unstaked(shared)
        .await
        .map_err(|e| ActionError::from_contract(e.into()))?;
    Ok(match outcome {
        CollectOutcome::Swept {
            tx_hash,
            swept,
            remainder,
            another_pool_remains,
        } => CollectReceipt {
            kind: "swept",
            tx_hash: Some(tx_hash.to_string()),
            swept: Some(swept.into()),
            remainder: Some(remainder.into()),
            another_pool_remains: Some(another_pool_remains),
        },
        CollectOutcome::NothingLeft => CollectReceipt {
            kind: "nothing_left",
            tx_hash: None,
            swept: None,
            remainder: None,
            another_pool_remains: None,
        },
    })
}
