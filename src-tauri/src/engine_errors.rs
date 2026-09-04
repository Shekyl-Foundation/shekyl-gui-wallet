// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Engine errors as operator text.
//!
//! A projection, and projections live outside the session shell
//! (`.cursor/rules/27-composition-decomposition.mdc`) — `engine_session`
//! runs the choreography; this says what went wrong in words the person at
//! the wallet can act on. Every arm names a remedy or says plainly that
//! nothing was written, because an error a user cannot act on is a failure
//! of the wallet, not of the user (rule 82).

use shekyl_engine_core::FirstStakeError;

pub(crate) fn map_open_err(e: shekyl_engine_core::OpenError) -> String {
    format!("wallet error: {e}")
}

pub(crate) fn map_first_stake_err(e: FirstStakeError) -> String {
    match e {
        FirstStakeError::BondInFlight => {
            "a signed bond post is already awaiting dispatch (stake in flight)".into()
        }
        FirstStakeError::AlreadyStaked => "this wallet is already an active staker".into(),
        FirstStakeError::Funding(detail) => {
            format!(
                "not ready to stake ({detail}); fund the persona (stake_in) and sync, then retry"
            )
        }
        FirstStakeError::FundingFragmented { max } => {
            // The one first-stake refusal that a retry cannot clear.
            // Neither standing funding remedy applies: the balance is
            // intact, so there is nothing to repair, and another transfer
            // in adds one more piece to the set that is already over the
            // limit. No consolidation path exists for a pool this
            // fragmented (core's bond assembly says so at the refusal), so
            // the text says that plainly instead of offering a retry that
            // would spend a fee to rediscover this same message (rule 82).
            format!(
                "your staking balance arrived in more separate transfers (more than \
                 {max}) than a single stake can gather at once; nothing was written \
                 and your funds were not touched. Moving more money into staking will \
                 not clear this — it adds another piece — and the wallet cannot yet \
                 combine them for you. When you next fund staking from a fresh \
                 balance, keep it to at most {max} transfers"
            )
        }
        FirstStakeError::FeeEstimate(_) => {
            "fee estimation failed; check the daemon connection and retry".into()
        }
        FirstStakeError::NoStakeEngine => {
            "stake engine not ready after intent open; retry activation".into()
        }
        FirstStakeError::WrongSlot { .. } => format!("stake: {e}"),
        FirstStakeError::State(d) => {
            format!("stake preflight failed ({d}); nothing durable was written")
        }
        FirstStakeError::Persist(d) | FirstStakeError::Engine(d) => {
            format!("stake failed mid-flow ({d}); call activate again to resume")
        }
        FirstStakeError::NoShardsAvailable => {
            // Market staking bonds over an assigned subset of the corpus and
            // the assignment mechanism is its own design round, so this is a
            // typed refusal, not a failure: nothing was written, nothing was
            // swept, and the wallet is exactly as it was.
            "archival staking is not open yet: market staking bonds over a shard \
             assigned automatically, and shard assignment is still being built. \
             Nothing was written and your funds were not touched"
                .into()
        }
        FirstStakeError::RecoveredPendingReopen => {
            "staking recovered an earlier attempt in this session; close and reopen \
             the wallet to finish, then check your staking status"
                .into()
        }
        FirstStakeError::FeeUnreasonable(v) => {
            format!(
                "the daemon quoted a bond fee outside the accepted range ({v}); \
                 nothing was written. Check that the daemon is one of yours and \
                 fully synced, then retry"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fragmented-funding refusal must not borrow the `Funding` arm's
    /// remedy, and must not end in a retry. Both arms are W1-clean funding
    /// refusals, which is exactly why the wrong one is tempting — but here
    /// the funding is intact and adding to it enlarges the set that caused
    /// the refusal, and no consolidation path exists yet, so every retry
    /// spends a fee to arrive back at this message (rule 82's misdiagnosis
    /// guard). Red if the arm is collapsed into the `Funding` text, and red
    /// if a retry imperative is ever appended to it.
    #[test]
    fn fragmented_funding_offers_neither_more_funding_nor_a_retry() {
        let msg = map_first_stake_err(FirstStakeError::FundingFragmented { max: 7 });
        // Copy guard, so it reads the copy the way a person does: the
        // message is several sentences, and a retry appended as a new one
        // would arrive capitalised.
        let copy = msg.to_lowercase();
        assert!(
            copy.contains("at most 7"),
            "the headroom renders, as the guidance the person can act on: {msg}"
        );
        assert!(
            !copy.contains("fund the persona"),
            "adding funding enlarges the eligible set; that remedy is a misdiagnosis: {msg}"
        );
        // Two stems, not a phrase list: every retry imperative this
        // projection speaks is built from one of them ("then retry", "and
        // retry", "retry activation", "call activate again to resume"),
        // whereas "try again"/"stake again" enumerate two of the verbs that
        // can precede "again" and miss the rest.
        for retry in ["retry", "again"] {
            assert!(
                !copy.contains(retry),
                "no retry clears this state — {retry:?} would cost a fee to rediscover \
                 the same refusal: {msg}"
            );
        }
        assert_ne!(
            msg,
            map_first_stake_err(FirstStakeError::Funding("not enough".into())),
            "the two funding refusals are distinct states and read differently"
        );
    }
}
