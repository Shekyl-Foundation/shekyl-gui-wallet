// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Staking money movement.
//!
//! `stake_in` returns the same reservation the send page confirms.
//! `drain` and `unstake` seal before they broadcast, so they share one
//! receipt: `BROADCAST`, or `ALREADY_IN_CHAIN` with the height the daemon
//! claimed. `collect_unstaked` is a different fact — a swept pass carries
//! its amounts, and an empty pool carries none. The page labels `unstake`
//! Release. The command name stays the contract's.

use serde::Serialize;
use shekyl_engine_core::{CollectOutcome, DrainOutcome, StakeFacade, UnstakeOutcome};
use shekyl_types::TxHash;
use shekyl_units::{AtomicUnits, AtomicUnitsString};
use tauri::State;

use crate::contract_error::{open_engine, ContractError};
use crate::send::BuiltPendingTx;
use crate::state::AppState;
use crate::validate;

/// Dispatch verdict shared by drain and release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DispatchVerdict {
    Broadcast,
    AlreadyInChain,
}

/// Receipt shared by `drain` and `unstake`.
///
/// `confirmed_height` is present only for [`DispatchVerdict::AlreadyInChain`].
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SealedReceipt {
    pub tx_hash: String,
    pub verdict: DispatchVerdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_height: Option<u64>,
}

/// `collect_unstaked`. A swept pass cannot omit either half of the
/// completion fact, and an empty pool cannot invent amounts.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectReceipt {
    Swept {
        tx_hash: String,
        swept: AtomicUnitsString,
        remainder: AtomicUnitsString,
        another_pool_remains: bool,
    },
    NothingLeft,
}

#[tauri::command]
pub async fn stake_in(
    state: State<'_, AppState>,
    amount: AtomicUnitsString,
) -> Result<BuiltPendingTx, ContractError> {
    let payment = stake_amount(amount)?;
    let shared = open_engine(&state).await?;
    let engine = shared.read().await;
    let pending = engine
        .stake()
        .stake_in(payment)
        .await
        .map_err(ContractError::from_engine)?;
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
) -> Result<SealedReceipt, ContractError> {
    let payment = amount.to_atomic_units();
    if payment.is_zero() {
        return Err(ContractError::invalid(
            "the amount must be greater than zero",
        ));
    }
    let shared = open_engine(&state).await?;
    let outcome = StakeFacade::drain_to_principal(shared, payment)
        .await
        .map_err(ContractError::from_engine)?;
    Ok(seal_drain(outcome))
}

#[tauri::command]
pub async fn unstake(state: State<'_, AppState>) -> Result<SealedReceipt, ContractError> {
    let shared = open_engine(&state).await?;
    let daemon = state.base_url().await;
    let outcome = StakeFacade::unstake(shared, &daemon)
        .await
        .map_err(ContractError::from_engine)?;
    Ok(seal_unstake(outcome))
}

#[tauri::command]
pub async fn collect_unstaked(state: State<'_, AppState>) -> Result<CollectReceipt, ContractError> {
    let shared = open_engine(&state).await?;
    let outcome = StakeFacade::collect_unstaked(shared)
        .await
        .map_err(ContractError::from_engine)?;
    Ok(match outcome {
        CollectOutcome::Swept {
            tx_hash,
            swept,
            remainder,
            another_pool_remains,
        } => CollectReceipt::Swept {
            tx_hash: tx_hash.to_string(),
            swept: swept.into(),
            remainder: remainder.into(),
            another_pool_remains,
        },
        CollectOutcome::NothingLeft => CollectReceipt::NothingLeft,
    })
}

/// A fund the command will reserve. Zero is invalid params, the same
/// sentence `validate_amount` gives a send, and it never reaches the engine.
fn stake_amount(amount: AtomicUnitsString) -> Result<AtomicUnits, ContractError> {
    let payment = amount.to_atomic_units();
    validate::validate_amount(payment.to_raw()).map_err(ContractError::invalid)?;
    Ok(payment)
}

fn seal(tx_hash: TxHash, confirmed_height: Option<u64>) -> SealedReceipt {
    SealedReceipt {
        tx_hash: tx_hash.to_string(),
        verdict: match confirmed_height {
            Some(_) => DispatchVerdict::AlreadyInChain,
            None => DispatchVerdict::Broadcast,
        },
        confirmed_height,
    }
}

fn seal_drain(outcome: DrainOutcome) -> SealedReceipt {
    match outcome {
        DrainOutcome::Broadcast { tx_hash } => seal(tx_hash, None),
        DrainOutcome::AlreadyInChain { tx_hash, height } => seal(tx_hash, Some(height)),
    }
}

fn seal_unstake(outcome: UnstakeOutcome) -> SealedReceipt {
    match outcome {
        UnstakeOutcome::Broadcast { tx_hash } => seal(tx_hash, None),
        UnstakeOutcome::AlreadyInChain { tx_hash, height } => seal(tx_hash, Some(height)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_fund_is_refused_before_a_reservation() {
        let err = stake_amount(AtomicUnits::ZERO.into()).expect_err("zero");
        assert_eq!(err.code, "INVALID_PARAMS");
        assert_eq!(
            err.message,
            "invalid params: Amount must be greater than zero"
        );
    }

    #[test]
    fn a_broadcast_receipt_omits_the_height() {
        let json = serde_json::to_value(seal(TxHash::from_bytes([0x11; 32]), None)).unwrap();
        assert_eq!(json["verdict"], "BROADCAST");
        assert!(json.get("confirmed_height").is_none());
    }

    #[test]
    fn an_already_in_chain_receipt_carries_the_height() {
        let json = serde_json::to_value(seal(TxHash::from_bytes([0x11; 32]), Some(42))).unwrap();
        assert_eq!(json["verdict"], "ALREADY_IN_CHAIN");
        assert_eq!(json["confirmed_height"], 42);
    }

    #[test]
    fn collect_nothing_left_has_no_amounts() {
        let json = serde_json::to_value(CollectReceipt::NothingLeft).unwrap();
        assert_eq!(json, serde_json::json!({ "status": "NOTHING_LEFT" }));
    }

    #[test]
    fn collect_swept_requires_both_completion_facts() {
        let json = serde_json::to_value(CollectReceipt::Swept {
            tx_hash: "ab".repeat(32),
            swept: AtomicUnits::from_raw(9).into(),
            remainder: AtomicUnits::ZERO.into(),
            another_pool_remains: true,
        })
        .unwrap();
        assert_eq!(json["status"], "SWEPT");
        assert_eq!(json["remainder"], "0");
        assert_eq!(json["another_pool_remains"], true);
        assert!(json.get("kind").is_none());
    }
}
