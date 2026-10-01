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

use shekyl_engine_core::{FirstStakeError, IoError, OpenError, RefreshError};
use shekyl_rpc_client::DaemonFault;
use shekyl_wallet_contract::error::WalletRpcError;

/// Open, create and close failures as operator text: the wallet contract's
/// message for the cause (`shekyl-wallet-contract`), the same words the
/// wallet RPC answers with. It names the remedy and carries no local path —
/// the engine's own rendering can ("lock held on {path}").
pub(crate) fn map_open_err(e: OpenError) -> String {
    WalletRpcError::from(e).message()
}

/// Refresh failures as operator text, by the same contract mapping: a
/// daemon that refuses on identity reads as its axis and remedy (another
/// network, version or chain), an outage as an outage.
pub(crate) fn map_refresh_err(e: RefreshError) -> String {
    WalletRpcError::from(e).message()
}

/// Whether a refresh failed because the daemon is not one this wallet can use
/// (`VC-4`). Read from the typed verdict the engine carries, never from text.
pub(crate) const fn is_identity_refusal(e: &RefreshError) -> bool {
    matches!(
        e,
        RefreshError::Io(IoError::Daemon {
            fault: DaemonFault::Identity(_),
            ..
        })
    )
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
        // The failed fee query's own cause, in the contract's words: no
        // answer, a daemon this wallet cannot use, or a reply it cannot read.
        FirstStakeError::FeeEstimate(e) => WalletRpcError::from(e).message(),
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
            // Engine `first_stake` still refuses Market until shard
            // assignment lands (its own round). The Shards page's selection
            // is session state only — never written, never a parameter of
            // `stake` (the contract takes a posture, not a shard set) — so
            // this arm must not describe it and must not claim the network
            // assigned anything.
            "archival staking is not open yet: the network does not assign \
             archives to stakers until shard assignment lands. Nothing was \
             written and your funds were not touched"
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

    #[test]
    fn no_shards_names_assignment_not_the_picker() {
        let msg = map_first_stake_err(FirstStakeError::NoShardsAvailable);
        let copy = msg.to_lowercase();
        assert!(
            copy.contains("not open yet") && copy.contains("shard assignment"),
            "the refusal is the network's assignment round, not a picker gap: {msg}"
        );
        assert!(
            !copy.contains("selected") && !copy.contains("selection"),
            "the Shards page selection is not a parameter of stake and must not be described: {msg}"
        );
        assert!(
            !copy.contains("assigned automatically"),
            "must not claim the network assigned the shard: {msg}"
        );
        assert!(
            copy.contains("nothing was written"),
            "funds-safe close: {msg}"
        );
    }

    use shekyl_rpc_client::{DaemonNetwork, IdentityMismatch};

    fn daemon_failure(fault: DaemonFault) -> RefreshError {
        RefreshError::Io(IoError::Daemon {
            fault,
            detail: "reply from /home/user/.shekyl: missing field".into(),
        })
    }

    /// A VC-4 refusal is recognised by its type, and reads as the axis and
    /// the remedy — both networks named — rather than as a refresh failure.
    #[test]
    fn an_identity_refusal_is_recognised_by_type_and_names_its_remedy() {
        let refused = daemon_failure(DaemonFault::Identity(IdentityMismatch::Network {
            ours: DaemonNetwork::Stagenet,
            theirs: DaemonNetwork::Mainnet,
        }));
        assert!(is_identity_refusal(&refused));
        let msg = map_refresh_err(refused);
        assert!(
            msg.contains("mainnet") && msg.contains("stagenet"),
            "the person sees which daemon to use instead: {msg}"
        );
    }

    /// Only an identity verdict is a refusal: a daemon that did not answer,
    /// or answered badly, is not a reason to close the wallet.
    #[test]
    fn an_outage_or_bad_reply_is_not_an_identity_refusal() {
        for fault in [DaemonFault::Unreachable, DaemonFault::Protocol] {
            let err = daemon_failure(fault);
            assert!(!is_identity_refusal(&err), "{fault:?}");
            let msg = map_refresh_err(err);
            assert!(
                !msg.contains("/home/user") && !msg.contains("missing field"),
                "the daemon's text and local paths stay in the log: {msg}"
            );
        }
    }

    /// An open failure reads as the contract's remedy for its cause, not the
    /// engine's rendering, which names the local path ("lock held on …").
    #[test]
    fn an_open_failure_names_its_remedy_without_a_path() {
        let locked = OpenError::Io(IoError::WalletFile(
            shekyl_engine_file::WalletFileError::AlreadyLocked {
                path: "/home/user/wallets/main.wallet.keys".into(),
            },
        ));
        assert!(
            locked.to_string().contains("/home/user"),
            "the engine's rendering is the leak this mapping closes"
        );
        let msg = map_open_err(locked);
        assert!(!msg.contains("/home/user"), "{msg}");
        assert!(
            msg.contains("another process"),
            "the remedy names the cause: {msg}"
        );
    }
}
