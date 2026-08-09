// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Project Engine receive-ledger + send-journal rows for the Transactions list.
//!
//! Mirrors the PR-SJ-2 merge in `shekyl-wallet-rpc` (`collect_transfers` /
//! `outgoing_transfer_view` / `transfer_state`) without depending on the RPC
//! crate. Divergences are intentional and narrow:
//!
//! - **No filters / attribution** — the GUI always shows the full list.
//! - **Newest-first display** — same order key as wallet-rpc (ascending
//!   inclusion height, incoming before outgoing, never-mined last), then
//!   reversed for the Transactions UI.
//! - **Flat Tauri DTO** — atomic `u64` amounts and snake_case enums instead of
//!   OpenAPI `TransferView` strings.
//!
//! A future shared crate (or engine-core helper) should own this once; until
//! then this module is the single GUI home for the projection so it does not
//! live inside the Engine session type.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;
use shekyl_engine_state::{InFlightSpendLocks, SendRecord, SendState, TransferDetails};

/// Lifecycle status on a projected history row (rule 82 — never collapse arms).
///
/// Outgoing arms map 1:1 from [`SendState`]. Incoming arms match wallet-rpc
/// `transfer_state`: spent / awaiting confirmation / confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Confirmed,
    Pending,
    Failed,
    Dropped,
    /// User-abandoned send (`abandon_tx`, PR-SJ-3; outgoing only). Distinct
    /// from [`Self::Dropped`]: the release came from user intent, not
    /// confirmed-absent evidence, and a late confirmation still flips the
    /// row to [`Self::Confirmed`] loudly rather than staying wrong.
    Abandoned,
    /// Receive-side output already spent on chain (incoming only).
    Spent,
}

/// Direction of a projected history row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    In,
    Out,
}

/// One row in the Transactions list (receive ledger or send journal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransferRow {
    /// Stable list key: `{hash}:{output_index}` (in) or bare `{hash}` (out).
    pub id: String,
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    /// Inclusion height, or `None` when the tx is not on chain (pending /
    /// failed / dropped sends). Matches wallet-rpc's absent `block_height`.
    pub height: Option<u64>,
    pub timestamp: u64,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub pqc_protected: bool,
}

/// Narrow receive facts so projection tests need no full `TransferDetails`
/// crypto fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFact {
    pub tx_hash: [u8; 32],
    pub output_index: u64,
    pub amount: u64,
    pub block_height: u64,
    pub spent: bool,
    pub awaiting_confirmation: bool,
}

impl IncomingFact {
    /// Extract the projection facts from a ledger row.
    ///
    /// PR-SJ-1b retired the persisted `awaiting_confirmation` field: the
    /// F14 lock is journal-derived on demand, so the caller derives
    /// `spend_locks` once (under its ledger guard) and threads it here —
    /// same shape as wallet-rpc's `transfer_state(td, spend_locks)`.
    pub fn from_details(td: &TransferDetails, spend_locks: &InFlightSpendLocks) -> Self {
        Self {
            tx_hash: td.tx_hash.to_bytes(),
            output_index: td.internal_output_index,
            amount: td.amount().to_raw(),
            block_height: td.block_height,
            spent: td.spent,
            awaiting_confirmation: spend_locks.contains(td.global_output_index),
        }
    }
}

/// Merge scan-ledger receipts and send-journal sends into one newest-first list.
///
/// Pure so unit tests can exercise the projection without an open Engine.
pub fn merge_transfer_history(
    incoming: impl IntoIterator<Item = IncomingFact>,
    journal_rows: &BTreeMap<[u8; 32], SendRecord>,
) -> Result<Vec<TransferRow>, String> {
    let mut keyed: Vec<(HistoryOrder, TransferRow)> = Vec::new();

    for fact in incoming {
        let row = project_incoming_row(&fact);
        keyed.push((
            HistoryOrder {
                block_height: Some(fact.block_height),
                outgoing: false,
                tx_hash: fact.tx_hash,
                output_index: fact.output_index,
            },
            row,
        ));
    }

    for (txid, record) in journal_rows {
        let row = project_outgoing_row(txid, record)?;
        keyed.push((
            HistoryOrder {
                block_height: outgoing_block_height(record),
                outgoing: true,
                tx_hash: *txid,
                output_index: 0,
            },
            row,
        ));
    }

    // Same key as wallet-rpc (ascending, unmined last), then reverse for the
    // Transactions "newest first" presentation.
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(keyed.into_iter().rev().map(|(_, row)| row).collect())
}

/// Project one ledger receive output (no folding — one row per output, like
/// wallet-rpc).
fn project_incoming_row(fact: &IncomingFact) -> TransferRow {
    let status = if fact.spent {
        TransferStatus::Spent
    } else if fact.awaiting_confirmation {
        TransferStatus::Pending
    } else {
        TransferStatus::Confirmed
    };
    let hash = hex::encode(fact.tx_hash);
    TransferRow {
        id: format!("{hash}:{}", fact.output_index),
        hash,
        amount: fact.amount,
        fee: 0,
        height: Some(fact.block_height),
        timestamp: 0,
        direction: TransferDirection::In,
        status,
        pqc_protected: true,
    }
}

/// Project one send-journal record as an outgoing [`TransferRow`].
fn project_outgoing_row(txid: &[u8; 32], record: &SendRecord) -> Result<TransferRow, String> {
    let sent = record.sent_amount().ok_or_else(|| {
        format!(
            "send journal row {} has recipient amounts that do not sum",
            hex::encode(txid)
        )
    })?;
    let (status, height) = match record.state {
        SendState::Dispatched => (TransferStatus::Pending, None),
        SendState::Confirmed { height } => (TransferStatus::Confirmed, Some(height)),
        SendState::TerminalRejected => (TransferStatus::Failed, None),
        SendState::PresumedDead => (TransferStatus::Dropped, None),
        SendState::Abandoned => (TransferStatus::Abandoned, None),
    };
    let hash = hex::encode(txid);
    Ok(TransferRow {
        id: hash.clone(),
        hash,
        amount: sent,
        fee: record.fee,
        height,
        timestamp: 0,
        direction: TransferDirection::Out,
        status,
        pqc_protected: true,
    })
}

/// Inclusion height of a send, or `None` when it is not on chain.
///
/// Only refresh-observed `Confirmed { height }` yields a height — never
/// `dispatched_at_height` (rule 82; same rationale as wallet-rpc).
fn outgoing_block_height(record: &SendRecord) -> Option<u64> {
    match record.state {
        SendState::Confirmed { height } => Some(height),
        SendState::Dispatched
        | SendState::TerminalRejected
        | SendState::PresumedDead
        | SendState::Abandoned => None,
    }
}

/// Deterministic merge order (wallet-rpc `TransferOrder`).
#[derive(Debug, PartialEq, Eq)]
struct HistoryOrder {
    block_height: Option<u64>,
    outgoing: bool,
    tx_hash: [u8; 32],
    output_index: u64,
}

impl Ord for HistoryOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.block_height, other.block_height) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| self.outgoing.cmp(&other.outgoing))
        .then_with(|| self.tx_hash.cmp(&other.tx_hash))
        .then_with(|| self.output_index.cmp(&other.output_index))
    }
}

impl PartialOrd for HistoryOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_state::SendRecipient;

    fn sample_record(state: SendState, fee: u64, amounts: &[u64]) -> SendRecord {
        SendRecord {
            dispatched_at_height: 10,
            fee,
            recipients: amounts
                .iter()
                .map(|&amount| SendRecipient {
                    address: "SkTestAddr".into(),
                    amount,
                })
                .collect(),
            change_amount: 0,
            inputs: vec![],
            lock_baseline: None,
            state,
        }
    }

    fn incoming(
        seed: u8,
        amount: u64,
        height: u64,
        output_index: u64,
        spent: bool,
        awaiting: bool,
    ) -> IncomingFact {
        IncomingFact {
            tx_hash: [seed; 32],
            output_index,
            amount,
            block_height: height,
            spent,
            awaiting_confirmation: awaiting,
        }
    }

    #[test]
    fn outgoing_dispatched_is_pending_with_no_height() {
        let txid = [0xabu8; 32];
        let row = project_outgoing_row(&txid, &sample_record(SendState::Dispatched, 100, &[1_000]))
            .expect("project");
        assert_eq!(row.direction, TransferDirection::Out);
        assert_eq!(row.status, TransferStatus::Pending);
        assert_eq!(row.height, None);
        assert_eq!(row.amount, 1_000);
        assert_eq!(row.fee, 100);
        assert_eq!(row.hash, hex::encode(txid));
        assert_eq!(row.id, row.hash);
    }

    #[test]
    fn outgoing_confirmed_carries_inclusion_height() {
        let row = project_outgoing_row(
            &[1u8; 32],
            &sample_record(SendState::Confirmed { height: 42 }, 7, &[500, 250]),
        )
        .expect("project");
        assert_eq!(row.status, TransferStatus::Confirmed);
        assert_eq!(row.height, Some(42));
        assert_eq!(row.amount, 750);
    }

    #[test]
    fn outgoing_failed_and_dropped_never_look_pending_or_confirmed() {
        let failed = project_outgoing_row(
            &[2u8; 32],
            &sample_record(SendState::TerminalRejected, 1, &[9]),
        )
        .expect("failed");
        assert_eq!(failed.status, TransferStatus::Failed);
        assert_eq!(failed.height, None);

        let dropped =
            project_outgoing_row(&[3u8; 32], &sample_record(SendState::PresumedDead, 1, &[9]))
                .expect("dropped");
        assert_eq!(dropped.status, TransferStatus::Dropped);
        assert_eq!(dropped.height, None);
    }

    /// PR-SJ-3: a user-abandoned send keeps its own arm — it must not read
    /// as dropped (evidence-based) or pending, and it carries no height.
    #[test]
    fn outgoing_abandoned_keeps_its_own_arm() {
        let row = project_outgoing_row(&[4u8; 32], &sample_record(SendState::Abandoned, 1, &[9]))
            .expect("abandoned");
        assert_eq!(row.status, TransferStatus::Abandoned);
        assert_eq!(row.height, None);
    }

    #[test]
    fn outgoing_rejects_overflowing_recipient_sum() {
        let bad = SendRecord {
            dispatched_at_height: 1,
            fee: 0,
            recipients: vec![
                SendRecipient {
                    address: "a".into(),
                    amount: u64::MAX,
                },
                SendRecipient {
                    address: "b".into(),
                    amount: 1,
                },
            ],
            change_amount: 0,
            inputs: vec![],
            lock_baseline: None,
            state: SendState::Dispatched,
        };
        let err = project_outgoing_row(&[0u8; 32], &bad).expect_err("overflow");
        assert!(err.contains("do not sum"), "{err}");
    }

    #[test]
    fn incoming_status_matches_wallet_rpc_arms() {
        let confirmed = project_incoming_row(&incoming(1, 10, 100, 0, false, false));
        assert_eq!(confirmed.status, TransferStatus::Confirmed);
        assert_eq!(confirmed.height, Some(100));
        assert_eq!(confirmed.id, format!("{}:0", hex::encode([1u8; 32])));

        let pending = project_incoming_row(&incoming(1, 10, 100, 1, false, true));
        assert_eq!(pending.status, TransferStatus::Pending);

        let spent = project_incoming_row(&incoming(1, 10, 100, 2, true, false));
        assert_eq!(spent.status, TransferStatus::Spent);
    }

    #[test]
    fn no_fold_keeps_one_row_per_output() {
        let facts = vec![
            incoming(0xAA, 100, 50, 0, false, false),
            incoming(0xAA, 200, 50, 1, false, false),
        ];
        let rows = merge_transfer_history(facts, &BTreeMap::new()).expect("merge");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount + rows[1].amount, 300);
        assert_ne!(rows[0].id, rows[1].id);
    }

    #[test]
    fn newest_first_is_reverse_of_wallet_rpc_order() {
        let mut journal = BTreeMap::new();
        journal.insert([0x11; 32], sample_record(SendState::Dispatched, 1, &[100]));
        journal.insert(
            [0x22; 32],
            sample_record(SendState::Confirmed { height: 5 }, 1, &[200]),
        );
        let facts = vec![incoming(0x33, 50, 5, 0, false, false)];

        let rows = merge_transfer_history(facts, &journal).expect("merge");
        assert_eq!(rows.len(), 3);
        // Ascending wallet-rpc order is [IN@5, OUT@5, unmined]; reverse →
        // unmined first, then OUT@5, then IN@5.
        assert_eq!(rows[0].status, TransferStatus::Pending);
        assert_eq!(rows[0].direction, TransferDirection::Out);
        assert_eq!(rows[1].direction, TransferDirection::Out);
        assert_eq!(rows[1].status, TransferStatus::Confirmed);
        assert_eq!(rows[1].height, Some(5));
        assert_eq!(rows[2].direction, TransferDirection::In);
        assert_eq!(rows[2].height, Some(5));
    }

    #[test]
    fn merge_fails_when_any_journal_row_overflows() {
        let mut journal = BTreeMap::new();
        journal.insert(
            [0x01; 32],
            SendRecord {
                dispatched_at_height: 1,
                fee: 0,
                recipients: vec![
                    SendRecipient {
                        address: "a".into(),
                        amount: u64::MAX,
                    },
                    SendRecipient {
                        address: "b".into(),
                        amount: 1,
                    },
                ],
                change_amount: 0,
                inputs: vec![],
                lock_baseline: None,
                state: SendState::Dispatched,
            },
        );
        let err = merge_transfer_history(std::iter::empty(), &journal).expect_err("overflow");
        assert!(err.contains("do not sum"), "{err}");
    }
}
