// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Receiving — payment requests and the `shekyl:` URI, under the wallet
//! contract's own names (WI-RPC-1): `create_payment_request`,
//! `list_payment_requests`, `parse_uri`. A feature module (rule 27) over
//! the same Engine calls as wallet-rpc.
//!
//! The contract also names `make_uri`, the freeform composer wallet-rpc and
//! the CLI use. This GUI has no such screen. Create and the list both
//! return the URI `Engine::format_request_uri` builds from the stored row,
//! so the link a person shows is the stored request and not a second
//! assembly of its fields. Registering `make_uri` with no page would fail
//! the command-surface consumer leg; it is in flight on the contract, not
//! a command here.
//!
//! Shekyl has no subaddresses and no accounts. A request is local
//! bookkeeping; its opaque `rid` rides the `shekyl:` URI. The scan matches
//! an inbound output whose encrypted label carries that `rid`, and a send
//! composed from the link echoes it (`send::build_pending_tx`), so a payment
//! between two of these wallets attributes on arrival. Only
//! `create_payment_request` mutates (persisted through the ledger's
//! crash-atomic save). Every atomic amount on this edge is
//! `wire::AtomicUnitsString`.

use serde::Serialize;
use shekyl_engine_core::{parse_payment_uri, NewPaymentRequest, PaymentRequestFilter};
use shekyl_engine_state::{PaymentRequest, PaymentRequestState};
use shekyl_types::Timestamp;
use tauri::State;

use crate::state::AppState;
use shekyl_units::{AtomicUnits, AtomicUnitsString};

/// The contract's names for a list filter. Unknown values are refused
/// without echoing the input, which a serde enum on the command argument
/// would put into the deserialize error.
pub mod contract {
    pub const FILTER_ALL: &str = "ALL";
    pub const FILTER_PENDING: &str = "PENDING";
    pub const FILTER_MATCHED: &str = "MATCHED";
}

/// The contract's `PaymentRequestState`, spelled the way the wire spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentRequestStateView {
    Pending,
    Matched,
    Expired,
    Cancelled,
}

impl From<PaymentRequestState> for PaymentRequestStateView {
    fn from(state: PaymentRequestState) -> Self {
        match state {
            PaymentRequestState::Pending => Self::Pending,
            PaymentRequestState::Matched => Self::Matched,
            PaymentRequestState::Expired => Self::Expired,
            PaymentRequestState::Cancelled => Self::Cancelled,
        }
    }
}

/// The contract's `PaymentRequest`, plus the link composed from that same
/// row. Bookkeeping facts only; no key material.
///
/// `uri` is not a field of the contract's `PaymentRequest`. It is the
/// string `Engine::format_request_uri` returns for this id, the same
/// string `create_payment_request` returns, so the client shows the stored
/// link instead of passing the row's fields back through a freeform
/// composer.
#[derive(Debug, Serialize)]
pub struct PaymentRequestView {
    /// Opaque request id (`rid`; non-zero u48, decimal string).
    pub id: String,
    pub label: String,
    pub amount: AtomicUnitsString,
    /// Wall-clock Unix seconds at creation (UTC).
    pub created_at: u64,
    /// Absolute expiry as Unix seconds (UTC), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
    pub state: PaymentRequestStateView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_output_index: Option<u64>,
    /// `shekyl:` link for this stored request.
    pub uri: String,
}

impl PaymentRequestView {
    fn from_stored(r: &PaymentRequest, uri: String) -> Self {
        Self {
            id: r.id.as_u64().to_string(),
            label: r.label.expose().as_str().to_owned(),
            amount: r.amount_atomic.into(),
            created_at: r.created_at.to_raw(),
            expiry: r.expiry.map(Timestamp::to_raw),
            state: r.state.into(),
            matched_tx_hash: r.matched_tx_hash.map(|h| h.to_string()),
            matched_output_index: r.matched_output_index.map(|i| i.to_raw()),
            uri,
        }
    }
}

/// `create_payment_request` result: the `rid` and the URI composed from the
/// STORED request, so the two cannot drift.
#[derive(Debug, Serialize)]
pub struct CreatedPaymentRequest {
    pub id: String,
    pub uri: String,
}

#[derive(Debug, Serialize)]
pub struct PaymentRequests {
    pub payment_requests: Vec<PaymentRequestView>,
}

/// `parse_uri` result: the query components as written. The address is not
/// format-validated here (the build validates it); nothing from the URI is
/// trusted beyond being text to show and prefill.
#[derive(Debug, Serialize)]
pub struct ParsedPaymentUri {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<AtomicUnitsString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
}

fn parse_filter(filter: Option<&str>) -> Result<PaymentRequestFilter, String> {
    match filter {
        None => Ok(PaymentRequestFilter::All),
        Some(contract::FILTER_ALL) => Ok(PaymentRequestFilter::All),
        Some(contract::FILTER_PENDING) => Ok(PaymentRequestFilter::Pending),
        Some(contract::FILTER_MATCHED) => Ok(PaymentRequestFilter::Matched),
        // Stable message; never reflect the supplied string.
        Some(_) => Err(format!(
            "unknown payment-request filter (expected {}, {} or {})",
            contract::FILTER_ALL,
            contract::FILTER_PENDING,
            contract::FILTER_MATCHED
        )),
    }
}

/// An invoice expiry: absolute Unix seconds. Values below 1e9 are refused as
/// height-shaped leftovers (RTN-6): invoice clocks are timestamps, not heights.
fn parse_expiry(secs: u64) -> Result<Timestamp, String> {
    Timestamp::from_invoice_unix(secs).ok_or_else(|| {
        "expiry must be unix seconds, not a chain height (values below 1e9 are refused)".into()
    })
}

fn unix_now() -> Timestamp {
    Timestamp::from_raw(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

/// Longest label a request may carry. The engine copies a non-empty label
/// onto the `shekyl:` link, so this is also the bound on that QR's text.
const MAX_LABEL_CHARS: usize = 256;

fn validate_label(label: &str) -> Result<(), String> {
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(format!(
            "label must be at most {MAX_LABEL_CHARS} characters"
        ));
    }
    if label.contains('\0') {
        return Err("label must not contain null bytes".into());
    }
    Ok(())
}

/// What `create_payment_request` will store and then put on the link.
fn validate_request_inputs(label: &str, amount: AtomicUnitsString) -> Result<(), String> {
    validate_label(label)?;
    crate::validate::validate_amount(amount.to_atomic_units().to_raw())
}

async fn shared_engine(state: &AppState) -> Result<crate::engine_session::SharedEngine, String> {
    if !*state.wallet_open.read().await {
        return Err("No wallet is open".into());
    }
    let eng = state.engine.lock().await;
    eng.shared_engine()
        .ok_or_else(|| "No wallet is open".into())
}

#[tauri::command]
pub async fn create_payment_request(
    state: State<'_, AppState>,
    label: String,
    amount: AtomicUnitsString,
    expiry: Option<u64>,
) -> Result<CreatedPaymentRequest, String> {
    validate_request_inputs(&label, amount)?;
    let expiry = expiry.map(parse_expiry).transpose()?;
    let shared = shared_engine(&state).await?;
    // Write guard: the one receiving method that mutates (local
    // bookkeeping, committed through the ledger's crash-atomic save).
    let engine = shared.write().await;
    let id = engine
        .create_payment_request_persisted(NewPaymentRequest {
            label,
            amount_atomic: amount.to_atomic_units(),
            created_at: unix_now(),
            expiry,
        })
        .map_err(|e| {
            // Persistence errors can carry filesystem paths; keep them in
            // the log and return a stable, detail-free message.
            tracing::warn!(error = %e, "create_payment_request: persist failed");
            "failed to save the payment request".to_string()
        })?;
    let address = engine
        .primary_address()
        .encode()
        .map_err(|e| format!("encode address: {e}"))?;
    // Composed from the stored request so the on-wire rid/amount/label
    // cannot drift from bookkeeping.
    let uri = engine
        .format_request_uri(&address, id)
        .ok_or_else(|| "created payment request not found".to_string())?;
    Ok(CreatedPaymentRequest {
        id: id.as_u64().to_string(),
        uri,
    })
}

#[tauri::command]
pub async fn list_payment_requests(
    state: State<'_, AppState>,
    filter: Option<String>,
) -> Result<PaymentRequests, String> {
    let filter = parse_filter(filter.as_deref())?;
    let shared = shared_engine(&state).await?;
    let engine = shared.read().await;
    let address = engine
        .primary_address()
        .encode()
        .map_err(|e| format!("encode address: {e}"))?;
    // Same composer as create, under this read guard: the id came from the
    // list just read, so a missing URI is an engine invariant break.
    let payment_requests = engine
        .list_payment_requests(filter)
        .iter()
        .map(|request| {
            let uri = engine
                .format_request_uri(&address, request.id)
                .ok_or_else(|| "payment request not found".to_string())?;
            Ok(PaymentRequestView::from_stored(request, uri))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(PaymentRequests { payment_requests })
}

/// Pure: needs no wallet. The URI is counterparty-controlled text.
#[tauri::command]
pub fn parse_uri(uri: String) -> Result<ParsedPaymentUri, String> {
    let parsed = parse_payment_uri(&uri).map_err(|e| format!("invalid payment URI: {e}"))?;
    Ok(ParsedPaymentUri {
        address: parsed.address,
        amount: parsed
            .amount_atomic
            .map(|a| AtomicUnits::from_raw(a).into()),
        label: parsed.label,
        rid: parsed.rid.map(|r| r.to_string()),
        expiry: parsed.expiry,
    })
}

#[cfg(test)]
mod tests {
    use shekyl_engine_core::format_payment_uri;

    use super::*;

    #[test]
    fn filters_are_the_contracts_names_and_nothing_else() {
        assert!(matches!(parse_filter(None), Ok(PaymentRequestFilter::All)));
        assert!(matches!(
            parse_filter(Some("ALL")),
            Ok(PaymentRequestFilter::All)
        ));
        assert!(matches!(
            parse_filter(Some("PENDING")),
            Ok(PaymentRequestFilter::Pending)
        ));
        assert!(matches!(
            parse_filter(Some("MATCHED")),
            Ok(PaymentRequestFilter::Matched)
        ));
        for bad in ["all", "Pending", "", "EXPIRED"] {
            let err = parse_filter(Some(bad)).unwrap_err();
            assert!(
                !err.contains(bad) || bad.is_empty(),
                "reflected the input: {err}"
            );
        }
    }

    #[test]
    fn request_inputs_are_bounded() {
        assert!(validate_request_inputs("rent", AtomicUnits::from_raw(1).into()).is_ok());
        assert!(validate_request_inputs(
            &"x".repeat(MAX_LABEL_CHARS),
            AtomicUnits::from_raw(1).into()
        )
        .is_ok());
        assert!(validate_request_inputs(
            &"x".repeat(MAX_LABEL_CHARS + 1),
            AtomicUnits::from_raw(1).into()
        )
        .is_err());
        assert!(validate_request_inputs("a\0b", AtomicUnits::from_raw(1).into()).is_err());
        assert!(
            validate_request_inputs("rent", AtomicUnits::from_raw(0).into()).is_err(),
            "a zero amount asks for nothing"
        );
    }

    #[test]
    fn expiry_is_a_timestamp_never_a_height() {
        assert!(parse_expiry(999_999_999).is_err());
        assert_eq!(parse_expiry(1_000_000_000).unwrap().to_raw(), 1_000_000_000);
    }

    #[test]
    fn parse_uri_round_trips_a_composed_link() {
        let uri = format_payment_uri(
            "shekyl1abc",
            Some((1u64 << 53) + 1),
            Some("rent"),
            Some(42),
            Some(1_700_000_000),
        );
        let parsed = parse_uri(uri).unwrap();
        assert_eq!(parsed.address, "shekyl1abc");
        assert_eq!(
            parsed.amount.unwrap().to_atomic_units().to_raw(),
            (1u64 << 53) + 1
        );
        assert_eq!(parsed.label.as_deref(), Some("rent"));
        assert_eq!(parsed.rid.as_deref(), Some("42"));
        assert_eq!(parsed.expiry, Some(1_700_000_000));
        assert!(parse_uri("http://example.com".into()).is_err());
    }

    #[test]
    fn view_serializes_the_contracts_shape() {
        let v = serde_json::to_value(PaymentRequestView {
            id: "42".into(),
            label: "rent".into(),
            amount: AtomicUnits::from_raw((1u64 << 53) + 1).into(),
            created_at: 1_700_000_000,
            expiry: None,
            state: PaymentRequestStateView::Pending,
            matched_tx_hash: None,
            matched_output_index: None,
            uri: "shekyl:shekyl1abc?rid=42".into(),
        })
        .unwrap();
        assert_eq!(v["amount"], "9007199254740993");
        assert_eq!(v["state"], "PENDING");
        assert_eq!(v["uri"], "shekyl:shekyl1abc?rid=42");
        assert!(v.get("expiry").is_none() && v.get("matched_tx_hash").is_none());
    }
}
