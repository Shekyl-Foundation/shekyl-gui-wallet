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

use shekyl_engine_core::{FirstStakeError, RefreshError};

pub(crate) fn map_open_err(e: shekyl_engine_core::OpenError) -> String {
    format!("wallet error: {e}")
}

/// Refresh failures as operator text.
///
/// A VC-4 identity refusal is already written for the person at the wallet
/// (`wallet_identity_message`); wrapping it in `refresh: daemon RPC failure:
/// invalid node (…)` would hide the axis and the remedy (rule 82). Other
/// refresh faults keep the `refresh:` prefix so they stay a sync problem.
pub(crate) fn map_refresh_err(e: RefreshError) -> String {
    let rendered = e.to_string();
    identity_refusal_message(&rendered).unwrap_or_else(|| format!("refresh: {e}"))
}

/// True when `err` is a VC-4 identity refusal, wrapped or already unwrapped.
pub(crate) fn is_identity_refusal(err: &str) -> bool {
    identity_refusal_message(err).is_some()
}

/// Pull the VC-4 operator sentence out of an `RpcError::InvalidNode` wrap.
///
/// `IoError::Daemon` stringifies as `daemon RPC failure: invalid node (MSG)`
/// and `RefreshError::Io` prefixes that. Other `InvalidNode` uses
/// ("invalid block", hex parse) never carry an identity-axis marker.
pub(crate) fn identity_refusal_message(err: &str) -> Option<String> {
    const PREFIX: &str = "invalid node (";
    let body = if let Some(start) = err.find(PREFIX) {
        let rest = &err[start + PREFIX.len()..];
        let end = rest.rfind(')')?;
        rest[..end].to_string()
    } else {
        err.to_string()
    };
    if is_identity_axis_sentence(&body) {
        Some(body)
    } else {
        None
    }
}

fn is_identity_axis_sentence(msg: &str) -> bool {
    msg.contains("RPC contract mismatch:")
        || msg.contains("consensus constants mismatch:")
        || msg.contains("genesis block mismatch:")
        || msg.contains("does not match the RPC contract")
        // VC-4 network axis, not `OpenError::NetworkMismatch` ("wallet file is").
        || (msg.contains("network mismatch:") && msg.contains("the daemon runs"))
}

pub(crate) fn map_first_stake_err(e: FirstStakeError, selected_shard_count: u32) -> String {
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
            // Engine `first_stake` still refuses Market until assignment
            // lands. The GUI selection is session state only — it is not
            // written, and this arm must not claim the network assigned
            // anything. Distinct copy for empty vs non-empty selection.
            if selected_shard_count == 0 {
                "no archives are selected; pick archives on the Shards page \
                 first. Nothing was written and your funds were not touched"
                    .into()
            } else {
                "archival staking is not open yet: a saved selection exists \
                 but posting is not open. Nothing was written and your funds \
                 were not touched"
                    .into()
            }
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
        let msg = map_first_stake_err(FirstStakeError::FundingFragmented { max: 7 }, 0);
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
            map_first_stake_err(FirstStakeError::Funding("not enough".into()), 0),
            "the two funding refusals are distinct states and read differently"
        );
    }

    #[test]
    fn no_shards_empty_selection_asks_to_pick() {
        let msg = map_first_stake_err(FirstStakeError::NoShardsAvailable, 0);
        let copy = msg.to_lowercase();
        assert!(
            copy.contains("no archives are selected"),
            "empty selection is a picker gap, not assignment: {msg}"
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

    #[test]
    fn no_shards_with_selection_says_posting_is_not_open() {
        let msg = map_first_stake_err(FirstStakeError::NoShardsAvailable, 3);
        let copy = msg.to_lowercase();
        assert!(
            copy.contains("saved selection exists"),
            "non-empty selection must not be described as missing: {msg}"
        );
        assert!(
            copy.contains("posting is not open"),
            "the refusal is posting, not the picker: {msg}"
        );
        assert_ne!(
            msg,
            map_first_stake_err(FirstStakeError::NoShardsAvailable, 0),
            "empty and non-empty selection are distinct copy"
        );
    }

    /// VC-4 identity refusals must be recognised through the refresh wrap
    /// (`daemon/scan IO failure: daemon RPC failure: invalid node (…)`),
    /// and must not match other `InvalidNode` uses or the file-level
    /// `OpenError::NetworkMismatch` sentence (same "network mismatch:" stem,
    /// different subject).
    #[test]
    fn identity_refusal_is_recognised_through_the_refresh_wrap() {
        let network = "refresh: daemon/scan IO failure: daemon RPC failure: invalid node \
             (network mismatch: this wallet is a mainnet wallet, the daemon runs \
             testnet. This is the case cross-cutting lock 5 names — a wallet \
             pointed at a daemon on another network — so it refuses rather than \
             scanning it.)";
        assert!(is_identity_refusal(network));
        assert!(
            map_refresh_err_from_display(network).starts_with("network mismatch:"),
            "the wrap is stripped so the person sees the axis first"
        );

        let unreadable = "daemon/scan IO failure: daemon RPC failure: invalid node \
             (this daemon's `get_version` does not match the RPC contract this \
             wallet was built against, so the two are on different RPC versions. \
             This wallet is 3.29. The reply could not be read, so the daemon's \
             version cannot be named here; align the two builds. (evidence: \
             missing field))";
        assert!(is_identity_refusal(unreadable));

        // RpcError::InvalidNode Display — the wrap `make_daemon`'s
        // handshake probe sees before any refresh prefix is applied.
        let rpc_wrap = "invalid node (network mismatch: this wallet is a mainnet wallet, \
             the daemon runs testnet. This is the case cross-cutting lock 5 names \
             — a wallet pointed at a daemon on another network — so it refuses \
             rather than scanning it.)";
        assert!(is_identity_refusal(rpc_wrap));
        assert!(
            identity_refusal_message(rpc_wrap)
                .as_deref()
                .is_some_and(|s| s.starts_with("network mismatch:")),
            "the probe wrap is stripped so create/restore see the axis first"
        );

        let other_invalid_node =
            "refresh: daemon/scan IO failure: daemon RPC failure: invalid node (invalid block)";
        assert!(
            !is_identity_refusal(other_invalid_node),
            "protocol InvalidNode is not an identity refusal: {other_invalid_node}"
        );

        let file_network = "wallet error: network mismatch: wallet file is mainnet, \
             daemon/caller expected testnet";
        assert!(
            !is_identity_refusal(file_network),
            "the envelope network check is a different refusal: {file_network}"
        );
    }

    fn map_refresh_err_from_display(rendered: &str) -> String {
        identity_refusal_message(rendered).unwrap_or_else(|| rendered.to_string())
    }
}
