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
