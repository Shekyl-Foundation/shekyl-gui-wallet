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
    FeePriority, InputCount, OutputCount, SubmitOutcome, TxRecipient, TxRequest,
};
use shekyl_units::AtomicUnits;
use tauri::State;

use crate::engine_session::SharedEngine;
use crate::state::AppState;
use crate::validate;

/// The contract's tier names (`wallet_rpc.yaml`, `build_pending_tx.priority`).
pub fn parse_priority(tier: &str) -> Result<FeePriority, SendError> {
    match tier {
        "ECONOMY" => Ok(FeePriority::Economy),
        "STANDARD" => Ok(FeePriority::Standard),
        "PRIORITY" => Ok(FeePriority::Priority),
        other => Err(SendError::invalid(format!(
            "unknown fee priority tier: {other} (expected ECONOMY, STANDARD or PRIORITY)"
        ))),
    }
}

/// `get_default_fee_priority` result: the daemon's tier quotes for the
/// canonical 2-in/2-out shape. An estimate for choosing a tier — never the
/// fee the user confirms; that comes from the build.
#[derive(Debug, Serialize)]
pub struct FeeTierQuote {
    pub default_priority: &'static str,
    pub economy_fee: u64,
    pub standard_fee: u64,
    pub priority_fee: u64,
    pub tree_depth: u8,
}

/// `build_pending_tx` result: the reservation the user is asked to confirm.
/// Only what consent is about — the exact fee for this transaction, and the
/// generation it was reviewed at.
#[derive(Debug, Serialize)]
pub struct BuiltPendingTx {
    /// Opaque reservation handle (`ReservationId::raw` as a decimal string).
    pub pending_tx_id: String,
    pub fee: u64,
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
            code: "INVALID_PARAMS",
            message,
            reservation_retained: false,
        }
    }

    fn wallet_closed() -> Self {
        Self {
            code: "WALLET_NOT_OPEN",
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
            SubmitError::ContentChanged { .. } => "CONTENT_GEN_MISMATCH",
            SubmitError::SnapshotInvalidated { .. } => "SNAPSHOT_INVALIDATED",
            SubmitError::ReservationNotFound { .. } => "RESERVATION_NOT_FOUND",
            SubmitError::DaemonAmbiguous { .. } => "SUBMIT_AMBIGUOUS",
            SubmitError::DaemonRejectedTerminal { .. }
            | SubmitError::DaemonRejectedRetryable { .. } => "SUBMIT_REJECTED",
            _ => "INTERNAL_ERROR",
        };
        Self {
            code,
            message: err.to_string(),
            reservation_retained: retained,
        }
    }
}

fn parse_pending_tx_id(s: &str) -> Result<shekyl_engine_core::ReservationId, SendError> {
    let raw: u64 = s
        .parse()
        .map_err(|_| SendError::invalid("pending_tx_id must be a decimal reservation id".into()))?;
    Ok(shekyl_engine_core::ReservationId::from_raw(raw))
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
        // The canonical 2-in/2-out shape, built the way wallet-rpc builds it.
        .quote_fee_tiers(InputCount::clamped(2), OutputCount::clamped(2))
        .await
        .map_err(|e| SendError {
            code: "FEE_ESTIMATION_FAILED",
            message: format!("fee quote: {e}"),
            reservation_retained: false,
        })?;
    Ok(FeeTierQuote {
        default_priority: "STANDARD",
        economy_fee: quote.economy_fee.to_raw(),
        standard_fee: quote.standard_fee.to_raw(),
        priority_fee: quote.priority_fee.to_raw(),
        tree_depth: quote.tree_depth,
    })
}

#[tauri::command]
pub async fn build_pending_tx(
    state: State<'_, AppState>,
    address: String,
    amount: u64,
    priority: String,
) -> Result<BuiltPendingTx, SendError> {
    validate::validate_address(&address).map_err(SendError::invalid)?;
    validate::validate_amount(amount).map_err(SendError::invalid)?;
    let priority = parse_priority(&priority)?;
    let shared = shared_engine(&state).await?;
    let request = TxRequest {
        recipients: vec![TxRecipient {
            address,
            amount_atomic_units: AtomicUnits::from_raw(amount),
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
                shekyl_engine_core::SendError::InsufficientFunds { .. } => "INSUFFICIENT_FUNDS",
                shekyl_engine_core::SendError::Fee(_) => "FEE_ESTIMATION_FAILED",
                _ => "INTERNAL_ERROR",
            },
            message: format!("build transaction: {e}"),
            reservation_retained: false,
        })?;
    Ok(BuiltPendingTx {
        pending_tx_id: pending.id.raw().to_string(),
        fee: pending.fee_atomic_units.to_raw(),
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
                SubmitOutcome::Accepted { .. } => ("ACCEPTED", None),
                SubmitOutcome::AlreadyInPool { .. } => ("ALREADY_IN_POOL", None),
                SubmitOutcome::AlreadyInChain { height, .. } => ("ALREADY_IN_CHAIN", Some(*height)),
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
        code: "INTERNAL_ERROR",
        message: format!("discard transaction: {e}"),
        reservation_retained: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_core::ReservationId;

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
            assert_eq!(err.code, "INVALID_PARAMS", "{bad}");
        }
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
