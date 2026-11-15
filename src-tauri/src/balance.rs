// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The contract's `get_balance` (`GetBalanceResult`), as the one wire DTO
//! projected from the engine's [`BalanceView`] — the same view wallet-rpc
//! serializes, so the desktop wallet and the RPC can never disagree about
//! what "spendable" or "staked" means (`StakeFacade::balance_view` owns the
//! projection; this module only spells it).
//!
//! Every amount is a decimal string (`shekyl_units::AtomicUnitsString`). The two
//! staking fields are **absent, never `"0"`,** when the sealed staking
//! state could not be read: the ledger's legs stay authoritative while the
//! staking projection degrades, and absence is structurally distinct from
//! a zero — a staker must never be shown "nothing staked" over a bad seal.
//! A non-staker's zeros are present and true.

use serde::Serialize;
use shekyl_engine_core::BalanceView;

use shekyl_units::AtomicUnitsString;

/// The contract's `GetBalanceResult` (0.19.0): the balance and each leg
/// once. `balance` = `spendable` + `incoming` + `parked`; `outgoing` and
/// `unspendable` sit beside it, never in it.
#[derive(Debug, Serialize)]
pub struct Balance {
    /// `spendable + incoming + parked`.
    pub balance: AtomicUnitsString,
    /// What a send could use now.
    pub spendable: AtomicUnitsString,
    /// Received and not yet spendable, plus in-flight sends' change.
    pub incoming: AtomicUnitsString,
    /// Blocks until the soonest on-chain incoming output is spendable;
    /// absent, not 0, when none is on chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incoming_wait: Option<u64>,
    /// The user's parked outputs: in the balance, out of coin selection.
    pub parked: AtomicUnitsString,
    /// Recipients plus fee of the in-flight sends. Beside the balance.
    pub outgoing: AtomicUnitsString,
    /// Received but never spendable by this wallet. Beside the balance.
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
        let legs = &view.legs;
        Self {
            balance: legs.balance().into(),
            spendable: legs.spendable.into(),
            incoming: legs.incoming.into(),
            incoming_wait: legs.incoming_wait.map(|w| w.to_raw()),
            parked: legs.parked.into(),
            outgoing: legs.outgoing.into(),
            unspendable: legs.unspendable.into(),
            staked: view.staking.map(|s| s.staked.into()),
            claimable_rewards: view.staking.map(|s| s.claimable_rewards.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_core::StakedTotals;
    use shekyl_scanner::BalanceSummary;
    use shekyl_units::AtomicUnits;

    fn view(staking: Option<StakedTotals>) -> BalanceView {
        BalanceView {
            legs: BalanceSummary {
                spendable: AtomicUnits::from_raw(40),
                incoming: AtomicUnits::from_raw(9),
                incoming_wait: None,
                parked: AtomicUnits::from_raw(2),
                outgoing: AtomicUnits::from_raw(5),
                unspendable: AtomicUnits::from_raw(7),
            },
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
                "balance": "51", "spendable": "40", "incoming": "9", "parked": "2",
                "outgoing": "5", "unspendable": "7",
                "staked": "9007199254740993", "claimable_rewards": "1234"
            })
        );
        let degraded = serde_json::to_value(Balance::from(view(None))).unwrap();
        assert!(degraded.get("staked").is_none() && degraded.get("claimable_rewards").is_none());
        assert!(degraded.get("incoming_wait").is_none(), "absent, not 0");
        assert_eq!(
            degraded["spendable"], "40",
            "the ledger's legs stay authoritative"
        );
    }
}
