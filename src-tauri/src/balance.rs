// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The contract's `get_balance` (`GetBalanceResult`), as the one wire DTO
//! projected from the engine's [`BalanceView`] — the same view wallet-rpc
//! serializes, so the desktop wallet and the RPC can never disagree about
//! what "liquid" or "staked" means (`StakeFacade::balance_view` owns the
//! projection; this module only spells it).
//!
//! Every amount is a decimal string (`wire::AtomicUnitsString`). The two
//! staking fields are **absent, never `"0"`,** when the sealed staking
//! state could not be read: the liquid fields stay authoritative while the
//! staking projection degrades, and absence is structurally distinct from
//! a zero — a staker must never be shown "nothing staked" over a bad seal.
//! A non-staker's zeros are present and true.

use serde::Serialize;
use shekyl_engine_core::BalanceView;

use crate::wire::AtomicUnitsString;

/// The contract's `GetBalanceResult`.
#[derive(Debug, Serialize)]
pub struct Balance {
    /// The one-glance figure. Engine-core's `project_balance`
    /// (`engine/balance_view.rs`, shekyl-core `48d515145`) assigns it and
    /// `unlocked` from the same ledger figure; the contract carries both so
    /// the engine can split them without a wire change.
    pub liquid: AtomicUnitsString,
    /// Spendable right now.
    pub unlocked: AtomicUnitsString,
    /// Committed to a send awaiting confirmation: counted, never spendable.
    pub pending: AtomicUnitsString,
    /// Received but never spendable by this wallet; counted nowhere else.
    pub unspendable: AtomicUnitsString,
    /// Bond principal under confirmed and in-flight bonds. Absent when the
    /// staking state could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staked: Option<AtomicUnitsString>,
    /// Emission-reward money received and still unspent. Absent exactly when
    /// `staked` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimable_rewards: Option<AtomicUnitsString>,
}

impl From<BalanceView> for Balance {
    fn from(view: BalanceView) -> Self {
        Self {
            liquid: view.liquid.into(),
            unlocked: view.unlocked.into(),
            pending: view.pending.into(),
            unspendable: view.unspendable.into(),
            staked: view.staking.map(|s| s.staked.into()),
            claimable_rewards: view.staking.map(|s| s.claimable_rewards.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_core::StakedTotals;
    use shekyl_units::AtomicUnits;

    fn view(staking: Option<StakedTotals>) -> BalanceView {
        BalanceView {
            liquid: AtomicUnits::from_raw(40),
            unlocked: AtomicUnits::from_raw(40),
            pending: AtomicUnits::from_raw(5),
            unspendable: AtomicUnits::from_raw(7),
            staking,
        }
    }

    #[test]
    fn spells_the_contract_and_keeps_absence_absent() {
        let present = serde_json::to_value(Balance::from(view(Some(StakedTotals {
            staked: AtomicUnits::from_raw((1u64 << 53) + 1),
            claimable_rewards: AtomicUnits::from_raw(1_234),
        }))))
        .unwrap();
        assert_eq!(
            present,
            serde_json::json!({
                "liquid": "40", "unlocked": "40", "pending": "5", "unspendable": "7",
                "staked": "9007199254740993", "claimable_rewards": "1234"
            })
        );
        let degraded = serde_json::to_value(Balance::from(view(None))).unwrap();
        assert!(degraded.get("staked").is_none() && degraded.get("claimable_rewards").is_none());
        assert_eq!(degraded["liquid"], "40", "liquid fields stay authoritative");
    }
}
