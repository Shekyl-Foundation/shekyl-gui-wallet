// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! The send flow — a feature module (rule 27), and the GUI's adoption of the
//! wallet contract's own three-step shape and names:
//!
//! `get_default_fee_priority` → `build_pending_tx` → `submit_pending_tx`
//! (or `discard_pending_tx`).
//!
//! What this replaces, and why. The one-shot `transfer` built and submitted in
//! a single call, and `estimate_fee` ran the FULL build — selection, `AssembleTx`,
//! FCMP++ proving, signing, reservation — on every 500 ms typing pause, then
//! discarded it. So every pause produced a complete signed transaction to the
//! typed address and amount; the fee the user saw belonged to a transaction
//! that was thrown away; `transfer` built a *different* one; and a submit-time
//! `ContentChanged` (the realized fee or change moved on re-anchor) was
//! resubmitted silently, with nobody on the other side of the consent the
//! engine exists to ask for.
//!
//! The invariant now: **one built transaction per user intent, and the fee the
//! user confirms is the fee of the transaction that ships.** The typing-time
//! figure is the daemon's tier quote for the canonical 2-in/2-out shape —
//! weight × rate, never a proof — fetched once per page, not per keystroke.
//! Review builds once and shows the exact fee. Confirm submits that
//! reservation with its `content_gen`. If the engine reports the content
//! changed, the reservation is discarded and rebuilt so the user re-confirms
//! numbers they can read: the engine exposes no view of a live reservation's
//! re-anchored fee, and asking for consent to a fee nobody can see would be the
//! one-shot defect with a button on it. Cancel, leaving the page, or closing the
//! window discards.
//!
//! Locking: the session's outer mutex is held only long enough to clone the
//! shared engine. The build and submit run under the engine's own read guard
//! (the same shape as wallet-rpc), so balance and status polling never wait
//! behind a proof.

use serde::Serialize;
use shekyl_engine_core::engine::SubmitError;
use shekyl_engine_core::{
    FeePriority, InputCount, OutputCount, ReservationId, SubmitOutcome, TxRecipient, TxRequest,
};
use tauri::State;

use crate::engine_session::SharedEngine;
use crate::state::AppState;
use crate::validate;
use crate::wire::AtomicUnitsString;

/// The wallet contract's vocabulary (`wallet_rpc.yaml`), so the page branches
/// on the same names the CLI and RPC clients see. Strings, not an enum, because
/// they cross the Tauri edge as the contract spells them.
pub mod contract {
    /// Fee tiers (`build_pending_tx.priority`).
    pub const TIER_ECONOMY: &str = "ECONOMY";
    pub const TIER_STANDARD: &str = "STANDARD";
    pub const TIER_PRIORITY: &str = "PRIORITY";
    /// The tier a fresh page starts on (`get_default_fee_priority.default_priority`).
    pub const DEFAULT_TIER: &str = TIER_STANDARD;

    /// Submit verdicts (`SubmitVerdictView`).
    pub const VERDICT_ACCEPTED: &str = "ACCEPTED";
    pub const VERDICT_ALREADY_IN_POOL: &str = "ALREADY_IN_POOL";
    pub const VERDICT_ALREADY_IN_CHAIN: &str = "ALREADY_IN_CHAIN";

    /// Error names (the contract's -29xxx / -32602 codes, by name).
    pub const ERR_INVALID_PARAMS: &str = "INVALID_PARAMS";
    pub const ERR_WALLET_NOT_OPEN: &str = "WALLET_NOT_OPEN";
    pub const ERR_INSUFFICIENT_FUNDS: &str = "INSUFFICIENT_FUNDS";
    pub const ERR_FEE_ESTIMATION_FAILED: &str = "FEE_ESTIMATION_FAILED";
    pub const ERR_RESERVATION_NOT_FOUND: &str = "RESERVATION_NOT_FOUND";
    pub const ERR_SNAPSHOT_INVALIDATED: &str = "SNAPSHOT_INVALIDATED";
    pub const ERR_CONTENT_GEN_MISMATCH: &str = "CONTENT_GEN_MISMATCH";
    pub const ERR_SUBMIT_REJECTED: &str = "SUBMIT_REJECTED";
    pub const ERR_SUBMIT_AMBIGUOUS: &str = "SUBMIT_AMBIGUOUS";
    /// The arms the contract leaves unnamed.
    pub const ERR_INTERNAL: &str = "INTERNAL_ERROR";
}

/// The shape the typing-time tier quote is priced for: one payment output and
/// its change, funded from two inputs — the canonical transfer, the same
/// default wallet-rpc's `get_default_fee_priority` uses when the caller gives
/// no shape.
const CANONICAL_INPUT_COUNT: usize = 2;
const CANONICAL_OUTPUT_COUNT: usize = 2;

/// The contract's tier names (`wallet_rpc.yaml`, `build_pending_tx.priority`).
pub fn parse_priority(tier: &str) -> Result<FeePriority, SendError> {
    match tier {
        contract::TIER_ECONOMY => Ok(FeePriority::Economy),
        contract::TIER_STANDARD => Ok(FeePriority::Standard),
        contract::TIER_PRIORITY => Ok(FeePriority::Priority),
        other => Err(SendError::invalid(format!(
            "unknown fee priority tier: {other} (expected {}, {} or {})",
            contract::TIER_ECONOMY,
            contract::TIER_STANDARD,
            contract::TIER_PRIORITY
        ))),
    }
}

/// `get_default_fee_priority` result: the daemon's tier quotes for the
/// canonical shape. An estimate for choosing a tier — never the fee the user
/// confirms; that comes from the build.
#[derive(Debug, Serialize)]
pub struct FeeTierQuote {
    pub default_priority: &'static str,
    pub economy_fee: AtomicUnitsString,
    pub standard_fee: AtomicUnitsString,
    pub priority_fee: AtomicUnitsString,
    pub tree_depth: u8,
}

/// `build_pending_tx` result: the reservation the user is asked to confirm.
/// Only what consent is about — the exact fee for this transaction, and the
/// generation it was reviewed at.
#[derive(Debug, Serialize)]
pub struct BuiltPendingTx {
    /// Opaque reservation handle (`ReservationId::raw` as a decimal string).
    pub pending_tx_id: String,
    /// The exact fee of this transaction.
    pub fee: AtomicUnitsString,
    /// Pass back as `seen_gen` on submit.
    pub content_gen: u64,
}

/// `submit_pending_tx` success verdict, 1:1 with the contract's
/// `SubmitVerdictView` strings.
#[derive(Debug, Serialize)]
pub struct SubmitResult {
    pub tx_hash: String,
    pub verdict: &'static str,
    /// Present iff `verdict` is `ALREADY_IN_CHAIN`: the daemon-claimed height,
    /// display metadata only — refresh remains the settlement authority.
    pub confirmed_height: Option<u64>,
}

/// The typed error the send commands reject with. `code` is the contract's
/// error name (`wallet_rpc.yaml`), so the page branches on the same vocabulary
/// the CLI and RPC clients see. `reservation_retained` tells the page whether
/// the engine still holds the reservation — when it does, the page must not
/// discard it: an ambiguous or still-pending submit may already be on the
/// network, and releasing the funds would open a double-spend path.
#[derive(Debug, Serialize)]
pub struct SendError {
    pub code: &'static str,
    pub message: String,
    pub reservation_retained: bool,
}

impl SendError {
    fn invalid(message: String) -> Self {
        Self {
            code: contract::ERR_INVALID_PARAMS,
            message,
            reservation_retained: false,
        }
    }

    fn wallet_closed() -> Self {
        Self {
            code: contract::ERR_WALLET_NOT_OPEN,
            message: "No wallet is open".into(),
            reservation_retained: false,
        }
    }

    /// Whether the engine keeps the reservation after this submit error, per
    /// `SubmitError`'s own documentation of each arm. Not unit-testable from
    /// this crate — the enum is `#[non_exhaustive]`, so no value of it can be
    /// constructed here — the page tests cover the policy through
    /// `reservation_retained`.
    pub fn reservation_retained_after(err: &SubmitError) -> bool {
        matches!(
            err,
            SubmitError::ContentChanged { .. }
                | SubmitError::DaemonAmbiguous { .. }
                | SubmitError::DaemonRejectedRetryable { .. }
                | SubmitError::SubmitAlreadyPending { .. }
        )
    }

    fn from_submit(err: SubmitError) -> Self {
        let retained = Self::reservation_retained_after(&err);
        // The contract's names (`wallet_rpc.yaml` -29103..-29107), folded the
        // way wallet-rpc folds them: both daemon-rejection arms are
        // SUBMIT_REJECTED, and the arms the contract leaves unnamed are
        // internal. `SubmitError` is `#[non_exhaustive]`, hence the wildcard.
        let code = match &err {
            SubmitError::ContentChanged { .. } => contract::ERR_CONTENT_GEN_MISMATCH,
            SubmitError::SnapshotInvalidated { .. } => contract::ERR_SNAPSHOT_INVALIDATED,
            SubmitError::ReservationNotFound { .. } => contract::ERR_RESERVATION_NOT_FOUND,
            SubmitError::DaemonAmbiguous { .. } => contract::ERR_SUBMIT_AMBIGUOUS,
            SubmitError::DaemonRejectedTerminal { .. }
            | SubmitError::DaemonRejectedRetryable { .. } => contract::ERR_SUBMIT_REJECTED,
            _ => contract::ERR_INTERNAL,
        };
        Self {
            code,
            message: err.to_string(),
            reservation_retained: retained,
        }
    }
}

fn parse_pending_tx_id(s: &str) -> Result<ReservationId, SendError> {
    let raw: u64 = s
        .parse()
        .map_err(|_| SendError::invalid("pending_tx_id must be a decimal reservation id".into()))?;
    Ok(ReservationId::from_raw(raw))
}

/// Clone the shared engine out from under the session lock, so the caller can
/// drop that lock before doing anything slow.
async fn shared_engine(state: &AppState) -> Result<SharedEngine, SendError> {
    if !*state.wallet_open.read().await {
        return Err(SendError::wallet_closed());
    }
    let eng = state.engine.lock().await;
    eng.shared_engine().ok_or_else(SendError::wallet_closed)
}

#[tauri::command]
pub async fn get_default_fee_priority(
    state: State<'_, AppState>,
) -> Result<FeeTierQuote, SendError> {
    let shared = shared_engine(&state).await?;
    let engine = shared.read().await;
    let quote = engine
        .quote_fee_tiers(
            InputCount::clamped(CANONICAL_INPUT_COUNT),
            OutputCount::clamped(CANONICAL_OUTPUT_COUNT),
        )
        .await
        .map_err(|e| SendError {
            code: contract::ERR_FEE_ESTIMATION_FAILED,
            message: format!("fee quote: {e}"),
            reservation_retained: false,
        })?;
    Ok(FeeTierQuote {
        default_priority: contract::DEFAULT_TIER,
        economy_fee: quote.economy_fee.into(),
        standard_fee: quote.standard_fee.into(),
        priority_fee: quote.priority_fee.into(),
        tree_depth: quote.tree_depth,
    })
}

#[tauri::command]
pub async fn build_pending_tx(
    state: State<'_, AppState>,
    address: String,
    amount: AtomicUnitsString,
    priority: String,
) -> Result<BuiltPendingTx, SendError> {
    validate::validate_address(&address).map_err(SendError::invalid)?;
    validate::validate_amount(amount.to_raw()).map_err(SendError::invalid)?;
    let priority = parse_priority(&priority)?;
    let shared = shared_engine(&state).await?;
    let request = TxRequest {
        recipients: vec![TxRecipient {
            address,
            amount_atomic_units: amount.to_atomic_units(),
        }],
        priority,
    };
    let engine = shared.read().await;
    let pending = engine
        .build_pending_tx_async(&request)
        .await
        .map_err(|e| SendError {
            // The contract's build-side names; anything else is internal.
            code: match &e {
                shekyl_engine_core::SendError::InsufficientFunds { .. } => {
                    contract::ERR_INSUFFICIENT_FUNDS
                }
                shekyl_engine_core::SendError::Fee(_) => contract::ERR_FEE_ESTIMATION_FAILED,
                _ => contract::ERR_INTERNAL,
            },
            message: format!("build transaction: {e}"),
            reservation_retained: false,
        })?;
    Ok(BuiltPendingTx {
        pending_tx_id: pending.id.raw().to_string(),
        fee: pending.fee_atomic_units.into(),
        content_gen: pending.content_gen,
    })
}

#[tauri::command]
pub async fn submit_pending_tx(
    state: State<'_, AppState>,
    pending_tx_id: String,
    seen_gen: u64,
) -> Result<SubmitResult, SendError> {
    let id = parse_pending_tx_id(&pending_tx_id)?;
    let shared = shared_engine(&state).await?;
    let engine = shared.read().await;
    match engine.submit_pending_tx_async(id, seen_gen).await {
        Ok(outcome) => {
            let (verdict, confirmed_height) = match &outcome {
                SubmitOutcome::Accepted { .. } => (contract::VERDICT_ACCEPTED, None),
                SubmitOutcome::AlreadyInPool { .. } => (contract::VERDICT_ALREADY_IN_POOL, None),
                SubmitOutcome::AlreadyInChain { height, .. } => {
                    (contract::VERDICT_ALREADY_IN_CHAIN, Some(*height))
                }
            };
            Ok(SubmitResult {
                tx_hash: outcome.hash().to_string(),
                verdict,
                confirmed_height,
            })
        }
        Err(err) => {
            let mapped = SendError::from_submit(err);
            if !mapped.reservation_retained {
                // The engine has already released or invalidated it; make the
                // release explicit so funds never stay locked behind a failed
                // submit the page has stopped tracking.
                let _ = engine.discard_pending_tx(id);
            }
            Err(mapped)
        }
    }
}

#[tauri::command]
pub async fn discard_pending_tx(
    state: State<'_, AppState>,
    pending_tx_id: String,
) -> Result<(), SendError> {
    let id = parse_pending_tx_id(&pending_tx_id)?;
    let shared = shared_engine(&state).await?;
    let engine = shared.read().await;
    engine.discard_pending_tx(id).map_err(|e| SendError {
        code: contract::ERR_INTERNAL,
        message: format!("discard transaction: {e}"),
        reservation_retained: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_are_the_contracts_names_and_nothing_else() {
        assert!(matches!(
            parse_priority("ECONOMY"),
            Ok(FeePriority::Economy)
        ));
        assert!(matches!(
            parse_priority("STANDARD"),
            Ok(FeePriority::Standard)
        ));
        assert!(matches!(
            parse_priority("PRIORITY"),
            Ok(FeePriority::Priority)
        ));
        for bad in ["standard", "Standard", "", "FAST", "2"] {
            let err = parse_priority(bad).unwrap_err();
            assert_eq!(err.code, contract::ERR_INVALID_PARAMS, "{bad}");
        }
    }

    /// The page never sees a JSON number for an amount: the DTOs the send
    /// commands return carry every atomic value as the contract's decimal
    /// string, so a fee above 2^53 reaches JS exactly.
    #[test]
    fn every_atomic_amount_on_the_wire_is_a_decimal_string() {
        const BEYOND_DOUBLE: u64 = (1u64 << 53) + 1;
        let built = serde_json::to_value(BuiltPendingTx {
            pending_tx_id: "42".into(),
            fee: BEYOND_DOUBLE.into(),
            content_gen: 0,
        })
        .unwrap();
        assert_eq!(built["fee"], "9007199254740993");
        assert_eq!(built["pending_tx_id"], "42");
        assert_eq!(built["content_gen"], 0);

        let quote = serde_json::to_value(FeeTierQuote {
            default_priority: contract::DEFAULT_TIER,
            economy_fee: 1.into(),
            standard_fee: 2.into(),
            priority_fee: BEYOND_DOUBLE.into(),
            tree_depth: 6,
        })
        .unwrap();
        assert_eq!(quote["default_priority"], "STANDARD");
        assert_eq!(quote["economy_fee"], "1");
        assert_eq!(quote["priority_fee"], "9007199254740993");
        assert_eq!(quote["tree_depth"], 6);
    }

    #[test]
    fn pending_tx_id_is_the_decimal_reservation_id() {
        assert_eq!(
            parse_pending_tx_id("42").unwrap(),
            ReservationId::from_raw(42)
        );
        assert_eq!(parse_pending_tx_id("x").unwrap_err().code, "INVALID_PARAMS");
    }
}
