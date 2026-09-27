// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Receiving — payment requests and the `shekyl:` URI, under the wallet
//! contract's own names (WI-RPC-1): `create_payment_request`,
//! `list_payment_requests`, `make_uri`, `parse_uri`. A feature module
//! (rule 27), the Tauri twin of wallet-rpc's `receiving.rs` over the same
//! Engine surface.
//!
//! Shekyl has **no subaddresses and no accounts**: the receive-attribution
//! surface is the payment request — local bookkeeping with an opaque `rid`
//! that rides the `shekyl:` URI, matched by the wallet's normal scan. Only
//! `create_payment_request` mutates (persisted through the ledger's
//! crash-atomic save); the other three are reads. Every atomic amount on
//! this edge is the contract's decimal string (`wire::AtomicUnitsString`).

use serde::Serialize;
use shekyl_engine_core::{
    format_payment_uri, parse_payment_uri, NewPaymentRequest, PaymentRequestFilter,
};
use shekyl_engine_state::{PaymentRequest, PaymentRequestId, PaymentRequestState};
use shekyl_types::Timestamp;
use tauri::State;

use crate::state::AppState;
use crate::wire::AtomicUnitsString;

/// The contract's names for a request's lifecycle (`PaymentRequestState`).
pub mod contract {
    pub const FILTER_ALL: &str = "ALL";
    pub const FILTER_PENDING: &str = "PENDING";
    pub const FILTER_MATCHED: &str = "MATCHED";

    pub const STATE_PENDING: &str = "PENDING";
    pub const STATE_MATCHED: &str = "MATCHED";
    pub const STATE_EXPIRED: &str = "EXPIRED";
    pub const STATE_CANCELLED: &str = "CANCELLED";
}

/// The contract's `PaymentRequest`: bookkeeping facts only, no key material.
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
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_output_index: Option<u64>,
}

impl From<&PaymentRequest> for PaymentRequestView {
    fn from(r: &PaymentRequest) -> Self {
        Self {
            id: r.id.as_u64().to_string(),
            label: r.label.expose().as_str().to_owned(),
            amount: r.amount_atomic.into(),
            created_at: r.created_at.to_raw(),
            expiry: r.expiry.map(Timestamp::to_raw),
            state: match r.state {
                PaymentRequestState::Pending => contract::STATE_PENDING,
                PaymentRequestState::Matched => contract::STATE_MATCHED,
                PaymentRequestState::Expired => contract::STATE_EXPIRED,
                PaymentRequestState::Cancelled => contract::STATE_CANCELLED,
            },
            matched_tx_hash: r.matched_tx_hash.map(|h| h.to_string()),
            matched_output_index: r.matched_output_index.map(|i| i.to_raw()),
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

#[derive(Debug, Serialize)]
pub struct PaymentUriResult {
    pub uri: String,
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

/// A `rid`: decimal string, non-zero, u48-fitting (the on-wire encoding);
/// anything else is refused rather than silently dropped.
fn parse_rid(s: &str) -> Result<u64, String> {
    let raw: u64 = s
        .parse()
        .map_err(|_| "rid must be a decimal integer string".to_string())?;
    if !PaymentRequestId::rid_fits_wire(raw) {
        return Err("rid must be non-zero and fit the u48 wire encoding".into());
    }
    Ok(raw)
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

fn validate_label(label: &str) -> Result<(), String> {
    const MAX_LABEL_CHARS: usize = 256;
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
    validate_label(&label)?;
    crate::validate::validate_amount(amount.to_raw())?;
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
    let payment_requests = engine
        .list_payment_requests(filter)
        .iter()
        .map(PaymentRequestView::from)
        .collect();
    Ok(PaymentRequests { payment_requests })
}

/// Freeform composition; `address` defaults to the open wallet's primary
/// address. The Receive page uses it to show a listed request's QR again.
#[tauri::command]
pub async fn make_uri(
    state: State<'_, AppState>,
    address: Option<String>,
    amount: Option<AtomicUnitsString>,
    label: Option<String>,
    rid: Option<String>,
    expiry: Option<u64>,
) -> Result<PaymentUriResult, String> {
    let rid = rid.as_deref().map(parse_rid).transpose()?;
    let expiry = expiry.map(parse_expiry).transpose()?.map(Timestamp::to_raw);
    let address = match address {
        // Refuse, never trim-and-repair: whitespace would embed into the URI.
        Some(a) if !a.is_empty() && !a.chars().any(char::is_whitespace) => a,
        Some(_) => return Err("address must be non-empty and contain no whitespace".into()),
        None => {
            let shared = shared_engine(&state).await?;
            let engine = shared.read().await;
            engine
                .primary_address()
                .encode()
                .map_err(|e| format!("encode address: {e}"))?
        }
    };
    let uri = format_payment_uri(
        &address,
        amount.map(AtomicUnitsString::to_raw),
        label.as_deref(),
        rid,
        expiry,
    );
    Ok(PaymentUriResult { uri })
}

/// Pure: needs no wallet. The URI is counterparty-controlled text.
#[tauri::command]
pub fn parse_uri(uri: String) -> Result<ParsedPaymentUri, String> {
    let parsed = parse_payment_uri(&uri).map_err(|e| format!("invalid payment URI: {e}"))?;
    Ok(ParsedPaymentUri {
        address: parsed.address,
        amount: parsed.amount_atomic.map(AtomicUnitsString::from),
        label: parsed.label,
        rid: parsed.rid.map(|r| r.to_string()),
        expiry: parsed.expiry,
    })
}

#[cfg(test)]
mod tests {
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
    fn rid_is_non_zero_and_fits_the_wire() {
        assert_eq!(parse_rid("1").unwrap(), 1);
        assert_eq!(parse_rid("281474976710655").unwrap(), (1u64 << 48) - 1);
        for bad in ["0", "281474976710656", "-1", "x", ""] {
            assert!(parse_rid(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn expiry_is_a_timestamp_never_a_height() {
        assert!(parse_expiry(999_999_999).is_err());
        assert_eq!(parse_expiry(1_000_000_000).unwrap().to_raw(), 1_000_000_000);
    }

    #[test]
    fn parse_uri_round_trips_what_make_uri_composes() {
        let uri = format_payment_uri(
            "shekyl1abc",
            Some((1u64 << 53) + 1),
            Some("rent"),
            Some(42),
            Some(1_700_000_000),
        );
        let parsed = parse_uri(uri).unwrap();
        assert_eq!(parsed.address, "shekyl1abc");
        assert_eq!(parsed.amount.unwrap().to_raw(), (1u64 << 53) + 1);
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
            amount: ((1u64 << 53) + 1).into(),
            created_at: 1_700_000_000,
            expiry: None,
            state: contract::STATE_PENDING,
            matched_tx_hash: None,
            matched_output_index: None,
        })
        .unwrap();
        assert_eq!(v["amount"], "9007199254740993");
        assert_eq!(v["state"], "PENDING");
        assert!(v.get("expiry").is_none() && v.get("matched_tx_hash").is_none());
    }
}
