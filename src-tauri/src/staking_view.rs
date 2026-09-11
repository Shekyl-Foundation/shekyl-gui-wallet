// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! WI-RPC-1 staking read projection for the GUI (GUI-PR3b).
//!
//! Single serializable DTO for the Tauri wire — the one projection of core
//! [`StakingReadView`] with newtypes unwrapped to raw integers. Lives here
//! (not inside [`crate::engine_session`]) so the session stays a thin shell
//! and so there is no second identity hop through `commands.rs`. Same
//! ownership pattern as [`crate::transfer_history`].
//!
//! The three balance legs stay distinct on purpose — confirmed bond
//! principal, pending (in-flight post) principal, and received-unspent
//! rewards are never conflated into one figure. Amounts are atomic-unit
//! `u64`s, display-only at the frontend (FOLLOWUPS "Atomic amounts
//! serialized as JS number"). Fail-closed: a corrupt / version-mismatched
//! seal is an `Err(String)` from the session method, never an empty view.

use serde::Serialize;
use shekyl_engine_core::{StakedOutput, StakingReadView};

/// Wire projection of core [`StakingReadView`] (WI-RPC-1; GUI-PR3b).
///
/// No `Clone`: no caller needs a second copy (rule 21).
#[derive(Debug, Serialize)]
pub struct StakingView {
    pub staking_enabled: bool,
    /// Bond principal locked under confirmed live bonds (atomic units).
    pub bonded_principal_confirmed: u64,
    /// Bond principal committed by in-flight (sealed, unconfirmed) posts.
    pub bonded_principal_pending: u64,
    /// Emission-reward money received and still unspent in `P`-owned outputs.
    pub rewards_received_unspent: u64,
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
}

/// One unspent staked (`P`-owned) funding output on the wire.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StakedOutputView {
    pub gindex: u64,
    /// Recovered cleartext amount, atomic units.
    pub amount: u64,
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
            bonded_principal_confirmed: view.balance.bonded_principal_confirmed.to_raw(),
            bonded_principal_pending: view.balance.bonded_principal_pending.to_raw(),
            rewards_received_unspent: view.balance.rewards_received_unspent.to_raw(),
            staked_outputs: view.outputs.iter().map(StakedOutputView::from).collect(),
            pscan_synced_height: view.pscan_synced_height.map(|h| h.to_raw()),
            recovery_pending_reopen: view.recovery_pending_reopen,
        }
    }
}

impl From<&StakedOutput> for StakedOutputView {
    fn from(o: &StakedOutput) -> Self {
        Self {
            gindex: o.gindex.to_raw(),
            amount: o.amount.to_raw(),
            p_slot: o.p_slot.to_raw(),
            unlock_height: o.unlock_height.to_raw(),
            confirmed: o.confirmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use shekyl_engine_core::StakedBalance;
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

    /// The three balance legs stay distinct across the projection — a swap
    /// (or a conflating sum) would misreport confirmed principal as pending
    /// or rewards, which the WI-RPC-1 shape exists to prevent.
    #[test]
    fn staking_view_keeps_balance_legs_distinct() {
        let view = StakingReadView {
            staking_enabled: true,
            balance: StakedBalance {
                bonded_principal_confirmed: AtomicUnits::from_raw(1_000),
                bonded_principal_pending: AtomicUnits::from_raw(2_000),
                rewards_received_unspent: AtomicUnits::from_raw(3_000),
            },
            outputs: Vec::new(),
            pscan_synced_height: None,
            recovery_pending_reopen: false,
        };
        let read = StakingView::from(view);
        assert!(read.staking_enabled);
        assert_eq!(read.bonded_principal_confirmed, 1_000);
        assert_eq!(read.bonded_principal_pending, 2_000);
        assert_eq!(read.rewards_received_unspent, 3_000);
        assert!(read.staked_outputs.is_empty());
        assert_eq!(read.pscan_synced_height, None);
    }

    #[test]
    fn staking_view_projects_outputs_and_frontier() {
        let view = StakingReadView {
            staking_enabled: true,
            balance: StakedBalance {
                bonded_principal_confirmed: AtomicUnits::ZERO,
                bonded_principal_pending: AtomicUnits::ZERO,
                rewards_received_unspent: AtomicUnits::ZERO,
            },
            outputs: vec![core_output(42, 5_000_000_000, 3, 12_345)],
            pscan_synced_height: Some(BlockHeight::from_raw(99_000)),
            recovery_pending_reopen: false,
        };
        let read = StakingView::from(view);
        assert_eq!(
            read.staked_outputs,
            vec![StakedOutputView {
                gindex: 42,
                amount: 5_000_000_000,
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
        let view = StakingReadView {
            staking_enabled: true,
            balance: StakedBalance {
                bonded_principal_confirmed: AtomicUnits::ZERO,
                bonded_principal_pending: AtomicUnits::ZERO,
                rewards_received_unspent: AtomicUnits::ZERO,
            },
            outputs: Vec::new(),
            pscan_synced_height: None,
            recovery_pending_reopen: true,
        };
        assert!(StakingView::from(view).recovery_pending_reopen);
    }
}
