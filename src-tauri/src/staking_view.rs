// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! WI-RPC-1 staking read projection for the GUI (GUI-PR3b).
//!
//! Single serializable DTO for the Tauri wire — the one projection of core
//! [`StakingReadView`]. Lives here (not inside [`crate::engine_session`]) so
//! the session stays a thin shell and so there is no second identity hop
//! through `commands.rs`. Same ownership pattern as [`crate::transfer_history`].
//!
//! The three balance legs stay distinct: confirmed bond principal, pending
//! (in-flight post) principal, and received-unspent rewards. Each of those
//! amounts, and each staked output's amount, is
//! [`AtomicUnitsString`] (a decimal string). Heights, slots, and
//! gindexes stay raw integers. `in_flight` and `outcomes` use the same
//! open-string spellings as `staking_info` (WR-Q1, WR-Q2); the sentences
//! come from the engine, and nothing here names a staking identity.
//! A corrupt or version-mismatched seal is an `Err(String)` from the
//! session method, never an empty view.

use serde::Serialize;
use shekyl_engine_core::{
    InFlightRecord, InFlightState, ReportedOutcome, StakedOutput, StakingReadView,
};
use shekyl_engine_state::pending_post_block::{OutcomeKind, RecordKind};

use shekyl_units::AtomicUnitsString;

/// Wire projection of core [`StakingReadView`] (WI-RPC-1; GUI-PR3b).
///
/// No `Clone`: no caller needs a second copy (rule 21).
#[derive(Debug, Serialize)]
pub struct StakingView {
    pub staking_enabled: bool,
    /// Bond principal locked under confirmed live bonds (atomic units).
    pub bonded_principal_confirmed: AtomicUnitsString,
    /// Bond principal committed by in-flight (sealed, unconfirmed) posts.
    pub bonded_principal_pending: AtomicUnitsString,
    /// Emission-reward money received and still unspent in `P`-owned outputs.
    pub rewards_received_unspent: AtomicUnitsString,
    /// Unspent `P`-owned funding outputs, in scan order.
    pub staked_outputs: Vec<StakedOutputView>,
    /// P-scan sealed frontier height (`None`: never scanned as `P`).
    pub pscan_synced_height: Option<u64>,
    /// The bond watch adopted a staked slot **this session**, and that slot
    /// cannot become operational until the wallet is reopened: staking
    /// operations against it fail until then. Carried to the frontend
    /// rather than dropped here, because a wallet that shows staker-hood it
    /// cannot act on is the failure this flag exists to prevent (rule 82).
    pub recovery_pending_reopen: bool,
    /// Every staking record still outstanding (WR-Q1). Empty means none.
    pub in_flight: Vec<InFlightView>,
    /// The last ending of each kind of staking action (WR-Q2). Empty means none.
    pub outcomes: Vec<StakingOutcomeView>,
}

/// One outstanding staking record. `kind` and `state` are open strings:
/// `stake` / `claim` / `drain` / `exit`, and `pending` / `dispatched` /
/// `refused` / `expired` / `held`. An unknown value is shown as itself.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct InFlightView {
    pub kind: String,
    pub state: String,
    /// Tip height of the first send. `None` when it was never sent.
    pub dispatched_at_height: Option<u64>,
    /// Sends so far, the first included. `0` when never sent.
    pub attempts: u32,
    /// For `refused` and `expired`, the block after which the transaction
    /// can no longer be mined. `None` in the other states.
    pub funds_return_at: Option<u64>,
    /// The engine's user sentence, when this state has one.
    pub reason: Option<String>,
}

/// How one kind of staking action last ended (WR-Q2). `outcome` is an open
/// string: `rejected`, `expired`, `ended_unconfirmed`, or `spent_elsewhere`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StakingOutcomeView {
    pub kind: String,
    pub outcome: String,
    /// The engine's user sentence. Never an enum name.
    pub reason: String,
    /// Tip height when the ending was written.
    pub at_height: u64,
    /// `returned`, or `none_held` when the record held no funds.
    pub funds: String,
}

/// One unspent staked (`P`-owned) funding output on the wire.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StakedOutputView {
    pub gindex: u64,
    /// Recovered cleartext amount, atomic units.
    pub amount: AtomicUnitsString,
    /// Owning persona's slot ordinal.
    pub p_slot: u32,
    /// Height at which the output becomes spendable.
    pub unlock_height: u64,
    /// Finality-confirmed (always `true` at V3.0; part of the wire shape).
    pub confirmed: bool,
}

impl From<StakingReadView> for StakingView {
    fn from(view: StakingReadView) -> Self {
        Self {
            staking_enabled: view.staking_enabled,
            bonded_principal_confirmed: view.balance.bonded_principal_confirmed.into(),
            bonded_principal_pending: view.balance.bonded_principal_pending.into(),
            rewards_received_unspent: view.balance.rewards_received_unspent.into(),
            staked_outputs: view.outputs.iter().map(StakedOutputView::from).collect(),
            pscan_synced_height: view.pscan_synced_height.map(|h| h.to_raw()),
            recovery_pending_reopen: view.recovery_pending_reopen,
            in_flight: view.in_flight.iter().map(in_flight_view).collect(),
            outcomes: view.outcomes.iter().map(outcome_view).collect(),
        }
    }
}

/// The wire name of a record kind. A release is `exit`, the word the user
/// sees, matching `staking_info`.
fn kind_wire(kind: RecordKind) -> &'static str {
    match kind {
        RecordKind::Post => "stake",
        RecordKind::Claim => "claim",
        RecordKind::Drain => "drain",
        RecordKind::Release => "exit",
    }
}

fn in_flight_view(record: &InFlightRecord) -> InFlightView {
    let (state, funds_return_at) = match record.state {
        InFlightState::Pending => ("pending", None),
        InFlightState::Dispatched => ("dispatched", None),
        InFlightState::Refused {
            funds_return_at, ..
        } => ("refused", Some(funds_return_at)),
        InFlightState::Expired {
            funds_return_at, ..
        } => ("expired", Some(funds_return_at)),
        InFlightState::Held => ("held", None),
    };
    InFlightView {
        kind: kind_wire(record.kind).to_owned(),
        state: state.to_owned(),
        dispatched_at_height: record.sends.map(|sends| sends.first_at_height.to_raw()),
        attempts: record.sends.map_or(0, |sends| sends.attempts),
        funds_return_at: funds_return_at.map(|height| height.to_raw()),
        reason: record.state.reason(),
    }
}

fn outcome_view(outcome: &ReportedOutcome) -> StakingOutcomeView {
    let name = match outcome.outcome {
        OutcomeKind::Rejected(_) => "rejected",
        OutcomeKind::Expired => "expired",
        OutcomeKind::EndedUnconfirmed(_) => "ended_unconfirmed",
        OutcomeKind::SpentElsewhere => "spent_elsewhere",
    };
    StakingOutcomeView {
        kind: kind_wire(outcome.kind).to_owned(),
        outcome: name.to_owned(),
        reason: outcome.reason(),
        at_height: outcome.at_height.to_raw(),
        funds: if outcome.returned_funds() {
            "returned"
        } else {
            "none_held"
        }
        .to_owned(),
    }
}

impl From<&StakedOutput> for StakedOutputView {
    fn from(o: &StakedOutput) -> Self {
        Self {
            gindex: o.gindex.to_raw(),
            amount: o.amount.into(),
            p_slot: o.p_slot.to_raw(),
            unlock_height: o.unlock_height.to_raw(),
            confirmed: o.confirmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use shekyl_engine_core::engine::staking_outcomes::Sends;
    use shekyl_engine_core::{InFlightRecord, InFlightState, ReportedOutcome, StakedBalance};
    use shekyl_engine_state::pending_post_block::{OutcomeKind, RecordKind, TerminalCause};
    use shekyl_types::{BlockHeight, GlobalOutputIndex, PSlot};
    use shekyl_units::AtomicUnits;

    fn core_output(gindex: u64, amount: u64, slot: u32, unlock: u64) -> StakedOutput {
        StakedOutput {
            gindex: GlobalOutputIndex::from_raw(gindex),
            amount: AtomicUnits::from_raw(amount),
            p_slot: PSlot::from_raw(slot),
            unlock_height: BlockHeight::from_raw(unlock),
            confirmed: true,
        }
    }

    /// The one `StakingReadView` literal. `StakingReadView` has no `Default`,
    /// and a default would compile straight through the next field core adds,
    /// which is how `in_flight` and `outcomes` would have been dropped from
    /// the projection without a test noticing. A new field fails here.
    fn staking_read(
        balance: StakedBalance,
        outputs: Vec<StakedOutput>,
        pscan_synced_height: Option<BlockHeight>,
        recovery_pending_reopen: bool,
    ) -> StakingReadView {
        StakingReadView {
            staking_enabled: true,
            balance,
            outputs,
            pscan_synced_height,
            recovery_pending_reopen,
            in_flight: Vec::new(),
            outcomes: Vec::new(),
        }
    }

    /// The three balance legs stay distinct across the projection — a swap
    /// (or a conflating sum) would misreport confirmed principal as pending
    /// or rewards, which the WI-RPC-1 shape exists to prevent.
    #[test]
    fn staking_view_keeps_balance_legs_distinct() {
        let view = staking_read(
            StakedBalance {
                bonded_principal_confirmed: AtomicUnits::from_raw(1_000),
                bonded_principal_pending: AtomicUnits::from_raw(2_000),
                rewards_received_unspent: AtomicUnits::from_raw(3_000),
            },
            Vec::new(),
            None,
            false,
        );
        let read = StakingView::from(view);
        assert!(read.staking_enabled);
        assert_eq!(
            read.bonded_principal_confirmed.to_atomic_units().to_raw(),
            1_000
        );
        assert_eq!(
            read.bonded_principal_pending.to_atomic_units().to_raw(),
            2_000
        );
        assert_eq!(
            read.rewards_received_unspent.to_atomic_units().to_raw(),
            3_000
        );
        assert!(read.staked_outputs.is_empty());
        assert_eq!(read.pscan_synced_height, None);
        assert!(read.in_flight.is_empty());
        assert!(read.outcomes.is_empty());
    }

    #[test]
    fn staking_view_projects_outputs_and_frontier() {
        let view = staking_read(
            StakedBalance {
                bonded_principal_confirmed: AtomicUnits::ZERO,
                bonded_principal_pending: AtomicUnits::ZERO,
                rewards_received_unspent: AtomicUnits::ZERO,
            },
            vec![core_output(42, 5_000_000_000, 3, 12_345)],
            Some(BlockHeight::from_raw(99_000)),
            false,
        );
        let read = StakingView::from(view);
        assert_eq!(
            read.staked_outputs,
            vec![StakedOutputView {
                gindex: 42,
                amount: AtomicUnits::from_raw(5_000_000_000).into(),
                p_slot: 3,
                unlock_height: 12_345,
                confirmed: true,
            }]
        );
        assert_eq!(read.pscan_synced_height, Some(99_000));
        assert!(!read.recovery_pending_reopen);
    }

    /// A slot the bond watch adopted this session cannot be acted on until
    /// the wallet is reopened, and the projection is what carries that to
    /// the panel that has to say so. The edit that turns this red is
    /// dropping the field from `From<StakingReadView>` — which is how the
    /// wallet would come to show staker-hood it cannot act on (rule 82).
    #[test]
    fn recovery_pending_reopen_reaches_the_wire() {
        let view = staking_read(
            StakedBalance {
                bonded_principal_confirmed: AtomicUnits::ZERO,
                bonded_principal_pending: AtomicUnits::ZERO,
                rewards_received_unspent: AtomicUnits::ZERO,
            },
            Vec::new(),
            None,
            true,
        );
        assert!(StakingView::from(view).recovery_pending_reopen);
    }

    /// Dropping `in_flight` or `outcomes` from `From` is how the panel would
    /// show a stake that is still outstanding, or one that ended, as if
    /// nothing had happened (WR-Q1, WR-Q2). The sentences are the engine's.
    #[test]
    fn in_flight_and_outcomes_reach_the_wire() {
        let refused = InFlightRecord {
            kind: RecordKind::Post,
            state: InFlightState::Refused {
                cause: TerminalCause::JoinMarketUnsupported,
                funds_return_at: BlockHeight::from_raw(5_100),
            },
            sends: Some(Sends {
                first_at_height: BlockHeight::from_raw(5_000),
                attempts: 2,
            }),
        };
        let rejected = ReportedOutcome {
            kind: RecordKind::Post,
            outcome: OutcomeKind::Rejected(TerminalCause::FeeTooLow),
            at_height: BlockHeight::from_raw(5_200),
            sent: true,
        };
        let unconfirmed = ReportedOutcome {
            kind: RecordKind::Claim,
            outcome: OutcomeKind::EndedUnconfirmed(None),
            at_height: BlockHeight::from_raw(5_300),
            sent: false,
        };
        let view = StakingReadView {
            in_flight: vec![refused],
            outcomes: vec![rejected, unconfirmed],
            ..staking_read(
                StakedBalance {
                    bonded_principal_confirmed: AtomicUnits::ZERO,
                    bonded_principal_pending: AtomicUnits::ZERO,
                    rewards_received_unspent: AtomicUnits::ZERO,
                },
                Vec::new(),
                None,
                false,
            )
        };
        let read = StakingView::from(view);
        assert_eq!(
            read.in_flight,
            vec![InFlightView {
                kind: "stake".to_owned(),
                state: "refused".to_owned(),
                dispatched_at_height: Some(5_000),
                attempts: 2,
                funds_return_at: Some(5_100),
                reason: refused.state.reason(),
            }]
        );
        assert_eq!(
            read.outcomes,
            vec![
                StakingOutcomeView {
                    kind: "stake".to_owned(),
                    outcome: "rejected".to_owned(),
                    reason: rejected.reason(),
                    at_height: 5_200,
                    funds: "returned".to_owned(),
                },
                StakingOutcomeView {
                    kind: "claim".to_owned(),
                    outcome: "ended_unconfirmed".to_owned(),
                    reason: unconfirmed.reason(),
                    at_height: 5_300,
                    funds: "none_held".to_owned(),
                },
            ]
        );
    }
}
