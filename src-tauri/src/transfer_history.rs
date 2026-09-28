// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Project Engine receive-ledger + send-journal rows for the Transactions list.
//!
//! Mirrors the wallet-rpc merge (`collect_transfers` / `outgoing_transfer_view`
//! / `transfer_state`) without depending on the RPC crate. This module is the
//! GUI's projection; the session calls it and does not own the DTO.
//!
//! Divergences from the contract's `Transfer`:
//!
//! - No filters, receive attribution, per-txid notes, or `spent_height` — the
//!   Transactions page does not render them.
//! - Newest-first display — same order key as wallet-rpc (ascending inclusion
//!   height, incoming before outgoing, never-mined last), then reversed.
//!
//! The JSON edge is [`TransferRow`]: amounts are [`crate::wire::AtomicUnitsString`]
//! (decimal strings), and `direction` / `state` / `unspendable_reason` use the
//! contract's `SCREAMING_SNAKE_CASE` spelling. Receive facts and the merge key
//! stay domain-typed (`TxHash`, `OutputIndexInTx`, `AtomicUnits`, `BlockHeight`,
//! the ledger's unspendable reason) until [`project_incoming_row`].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;
use shekyl_engine_state::{
    InFlightSpendLocks, SendRecord, SendState, TransferDetails,
    UnspendableReason as LedgerUnspendableReason,
};
use shekyl_types::{BlockHeight, OutputIndexInTx, TxHash};
use shekyl_units::AtomicUnits;

use crate::wire::AtomicUnitsString;

/// Lifecycle state on a projected history row (rule 82 — never collapse arms),
/// spelled as the contract's `Transfer.state` enum.
///
/// Outgoing arms map 1:1 from [`SendState`]. Incoming arms follow wallet-rpc
/// `transfer_state`: spent, then received-but-unspendable, then awaiting
/// confirmation, then confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferState {
    Confirmed,
    Pending,
    Failed,
    Dropped,
    /// User-abandoned send (`abandon_tx`; outgoing only). Distinct from
    /// [`Self::Dropped`]: the release came from user intent, not
    /// confirmed-absent evidence, and a late confirmation still flips the
    /// row to [`Self::Confirmed`].
    Abandoned,
    /// Receive-side output already spent on chain (incoming only).
    Spent,
    /// Receive-side output whose chain leaf can never be spent (incoming only).
    /// [`TransferRow::unspendable_reason`] names which half failed.
    Unspendable,
}

/// Why an incoming row can never be spent. Spelled as the contract's
/// `Transfer.unspendable_reason`. The ledger enum stays on [`IncomingFact`];
/// this is the wire spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnspendableReason {
    /// The published leaf does not open to this wallet's derivation.
    PqcLeafMismatch,
    /// The transaction carries no leaf entry for this output.
    PqcLeafEntryAbsent,
}

impl From<LedgerUnspendableReason> for UnspendableReason {
    fn from(reason: LedgerUnspendableReason) -> Self {
        match reason {
            LedgerUnspendableReason::PqcLeafMismatch => Self::PqcLeafMismatch,
            LedgerUnspendableReason::PqcLeafEntryAbsent => Self::PqcLeafEntryAbsent,
        }
    }
}

/// Direction of a projected history row, spelled as the contract's
/// `Transfer.direction` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferDirection {
    Incoming,
    Outgoing,
}

/// One row in the Transactions list: the contract fields the page renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransferRow {
    /// Stable list key: `{tx_hash}:{output_index}` (incoming) or bare
    /// `{tx_hash}` (outgoing). `tx_hash` alone is not unique across rows.
    pub id: String,
    pub tx_hash: String,
    pub amount: AtomicUnitsString,
    pub fee: AtomicUnitsString,
    /// Inclusion height; absent exactly when the tx is not on chain (pending /
    /// failed / dropped / abandoned sends), as the contract specifies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u64>,
    pub direction: TransferDirection,
    pub state: TransferState,
    /// Present exactly when [`Self::state`] is [`TransferState::Unspendable`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unspendable_reason: Option<UnspendableReason>,
}

/// Narrow receive facts so projection tests need no full `TransferDetails`
/// crypto fixtures. Domain-typed on purpose: this is still Rust, not the
/// Tauri JSON edge — that unwrap happens in [`project_incoming_row`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFact {
    pub tx_hash: TxHash,
    pub output_index: OutputIndexInTx,
    pub amount: AtomicUnits,
    pub block_height: BlockHeight,
    pub spent: bool,
    pub awaiting_confirmation: bool,
    /// Ledger classification. Converted to the wire enum in
    /// [`project_incoming_row`]; `None` when the output opened to this wallet.
    pub unspendable: Option<LedgerUnspendableReason>,
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
            tx_hash: td.tx_hash,
            output_index: td.internal_output_index,
            amount: td.amount(),
            block_height: td.block_height,
            spent: td.spent,
            awaiting_confirmation: spend_locks.contains(td.global_output_index),
            unspendable: td.unspendable,
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
                tx_hash: TxHash::from_bytes(*txid),
                output_index: OutputIndexInTx::ZERO,
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
/// Incoming settlement, in wallet-rpc order.
///
/// Spent wins, then the scan-time unspendable verdict (it outranks a lock
/// and `CONFIRMED`, which promises spendability), then awaiting confirmation.
/// The reason is returned only for [`TransferState::Unspendable`]: the
/// contract field is present exactly then, so a spent row omits it.
fn incoming_settlement(fact: &IncomingFact) -> (TransferState, Option<UnspendableReason>) {
    if fact.spent {
        (TransferState::Spent, None)
    } else if let Some(reason) = fact.unspendable {
        (
            TransferState::Unspendable,
            Some(UnspendableReason::from(reason)),
        )
    } else if fact.awaiting_confirmation {
        (TransferState::Pending, None)
    } else {
        (TransferState::Confirmed, None)
    }
}

fn project_incoming_row(fact: &IncomingFact) -> TransferRow {
    let (state, unspendable_reason) = incoming_settlement(fact);
    let tx_hash = fact.tx_hash.to_string();
    TransferRow {
        id: format!("{tx_hash}:{}", fact.output_index.to_raw()),
        tx_hash,
        amount: fact.amount.into(),
        fee: 0.into(),
        block_height: Some(fact.block_height.to_raw()),
        direction: TransferDirection::Incoming,
        state,
        unspendable_reason,
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
    let (state, block_height) = match record.state {
        SendState::Dispatched => (TransferState::Pending, None),
        SendState::Confirmed { height } => (TransferState::Confirmed, Some(height.to_raw())),
        SendState::TerminalRejected => (TransferState::Failed, None),
        SendState::PresumedDead => (TransferState::Dropped, None),
        SendState::Abandoned => (TransferState::Abandoned, None),
    };
    let tx_hash = hex::encode(txid);
    Ok(TransferRow {
        id: tx_hash.clone(),
        tx_hash,
        amount: sent.into(),
        fee: record.fee.into(),
        block_height,
        direction: TransferDirection::Outgoing,
        state,
        unspendable_reason: None,
    })
}

/// Inclusion height of a send, or `None` when it is not on chain.
///
/// Only refresh-observed `Confirmed { height }` yields a height — never
/// `dispatched_at_height` (rule 82; same rationale as wallet-rpc).
fn outgoing_block_height(record: &SendRecord) -> Option<BlockHeight> {
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
    block_height: Option<BlockHeight>,
    outgoing: bool,
    tx_hash: TxHash,
    output_index: OutputIndexInTx,
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
            dispatched_at_height: BlockHeight::from_raw(10),
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
            tx_hash: TxHash::from_bytes([seed; 32]),
            output_index: OutputIndexInTx::from_raw(output_index),
            amount: AtomicUnits::from_raw(amount),
            block_height: BlockHeight::from_raw(height),
            spent,
            awaiting_confirmation: awaiting,
            unspendable: None,
        }
    }

    #[test]
    fn outgoing_dispatched_is_pending_with_no_height() {
        let txid = [0xabu8; 32];
        let row = project_outgoing_row(&txid, &sample_record(SendState::Dispatched, 100, &[1_000]))
            .expect("project");
        assert_eq!(row.direction, TransferDirection::Outgoing);
        assert_eq!(row.state, TransferState::Pending);
        assert_eq!(row.block_height, None);
        assert_eq!(row.amount.to_raw(), 1_000);
        assert_eq!(row.fee.to_raw(), 100);
        assert_eq!(row.tx_hash, hex::encode(txid));
        assert_eq!(row.id, row.tx_hash);
        assert_eq!(row.unspendable_reason, None);
    }

    #[test]
    fn outgoing_confirmed_carries_inclusion_height() {
        let row = project_outgoing_row(
            &[1u8; 32],
            &sample_record(
                SendState::Confirmed {
                    height: BlockHeight::from_raw(42),
                },
                7,
                &[500, 250],
            ),
        )
        .expect("project");
        assert_eq!(row.state, TransferState::Confirmed);
        assert_eq!(row.block_height, Some(42));
        assert_eq!(row.amount.to_raw(), 750);
    }

    #[test]
    fn outgoing_failed_and_dropped_never_look_pending_or_confirmed() {
        let failed = project_outgoing_row(
            &[2u8; 32],
            &sample_record(SendState::TerminalRejected, 1, &[9]),
        )
        .expect("failed");
        assert_eq!(failed.state, TransferState::Failed);
        assert_eq!(failed.block_height, None);

        let dropped =
            project_outgoing_row(&[3u8; 32], &sample_record(SendState::PresumedDead, 1, &[9]))
                .expect("dropped");
        assert_eq!(dropped.state, TransferState::Dropped);
        assert_eq!(dropped.block_height, None);
    }

    /// PR-SJ-3: a user-abandoned send keeps its own arm — it must not read
    /// as dropped (evidence-based) or pending, and it carries no height.
    #[test]
    fn outgoing_abandoned_keeps_its_own_arm() {
        let row = project_outgoing_row(&[4u8; 32], &sample_record(SendState::Abandoned, 1, &[9]))
            .expect("abandoned");
        assert_eq!(row.state, TransferState::Abandoned);
        assert_eq!(row.block_height, None);
    }

    #[test]
    fn outgoing_rejects_overflowing_recipient_sum() {
        let bad = SendRecord {
            dispatched_at_height: BlockHeight::from_raw(1),
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
        assert_eq!(confirmed.state, TransferState::Confirmed);
        assert_eq!(confirmed.block_height, Some(100));
        assert_eq!(confirmed.id, format!("{}:0", hex::encode([1u8; 32])));

        let pending = project_incoming_row(&incoming(1, 10, 100, 1, false, true));
        assert_eq!(pending.state, TransferState::Pending);

        let spent = project_incoming_row(&incoming(1, 10, 100, 2, true, false));
        assert_eq!(spent.state, TransferState::Spent);
        assert_eq!(spent.unspendable_reason, None);
    }

    /// The scan-time verdict outranks a lock and `CONFIRMED`. Spent still
    /// wins, and then the reason is omitted: the contract field is present
    /// exactly when the state is `UNSPENDABLE`.
    #[test]
    fn incoming_unspendable_names_the_reason_and_spent_omits_it() {
        let mut mismatch = incoming(1, 10, 100, 0, false, true);
        mismatch.unspendable = Some(LedgerUnspendableReason::PqcLeafMismatch);
        let row = project_incoming_row(&mismatch);
        assert_eq!(row.state, TransferState::Unspendable);
        assert_eq!(
            row.unspendable_reason,
            Some(UnspendableReason::PqcLeafMismatch)
        );
        let json = serde_json::to_value(&row).expect("serialize");
        assert_eq!(json["state"], "UNSPENDABLE");
        assert_eq!(json["unspendable_reason"], "PQC_LEAF_MISMATCH");

        let mut absent = incoming(2, 10, 100, 0, false, false);
        absent.unspendable = Some(LedgerUnspendableReason::PqcLeafEntryAbsent);
        let row = project_incoming_row(&absent);
        assert_eq!(
            row.unspendable_reason,
            Some(UnspendableReason::PqcLeafEntryAbsent)
        );
        assert_eq!(
            serde_json::to_value(&row).expect("serialize")["unspendable_reason"],
            "PQC_LEAF_ENTRY_ABSENT"
        );

        let mut spent = incoming(3, 10, 100, 0, true, true);
        spent.unspendable = Some(LedgerUnspendableReason::PqcLeafMismatch);
        let row = project_incoming_row(&spent);
        assert_eq!(row.state, TransferState::Spent);
        assert_eq!(row.unspendable_reason, None);
        assert!(serde_json::to_value(&row)
            .expect("serialize")
            .get("unspendable_reason")
            .is_none());

        let confirmed = project_incoming_row(&incoming(4, 10, 100, 0, false, false));
        assert!(serde_json::to_value(&confirmed)
            .expect("serialize")
            .get("unspendable_reason")
            .is_none());
    }

    #[test]
    fn no_fold_keeps_one_row_per_output() {
        let facts = vec![
            incoming(0xAA, 100, 50, 0, false, false),
            incoming(0xAA, 200, 50, 1, false, false),
        ];
        let rows = merge_transfer_history(facts, &BTreeMap::new()).expect("merge");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].amount.to_raw() + rows[1].amount.to_raw(), 300);
        assert_ne!(rows[0].id, rows[1].id);
    }

    #[test]
    fn newest_first_is_reverse_of_wallet_rpc_order() {
        let mut journal = BTreeMap::new();
        journal.insert([0x11; 32], sample_record(SendState::Dispatched, 1, &[100]));
        journal.insert(
            [0x22; 32],
            sample_record(
                SendState::Confirmed {
                    height: BlockHeight::from_raw(5),
                },
                1,
                &[200],
            ),
        );
        let facts = vec![incoming(0x33, 50, 5, 0, false, false)];

        let rows = merge_transfer_history(facts, &journal).expect("merge");
        assert_eq!(rows.len(), 3);
        // Ascending wallet-rpc order is [IN@5, OUT@5, unmined]; reverse →
        // unmined first, then OUT@5, then IN@5.
        assert_eq!(rows[0].state, TransferState::Pending);
        assert_eq!(rows[0].direction, TransferDirection::Outgoing);
        assert_eq!(rows[1].direction, TransferDirection::Outgoing);
        assert_eq!(rows[1].state, TransferState::Confirmed);
        assert_eq!(rows[1].block_height, Some(5));
        assert_eq!(rows[2].direction, TransferDirection::Incoming);
        assert_eq!(rows[2].block_height, Some(5));
    }

    #[test]
    fn merge_fails_when_any_journal_row_overflows() {
        let mut journal = BTreeMap::new();
        journal.insert(
            [0x01; 32],
            SendRecord {
                dispatched_at_height: BlockHeight::from_raw(1),
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
