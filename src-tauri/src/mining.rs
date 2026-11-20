// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The node's miner: `mine_status`, `mine_start` and `mine_stop` on the 4.0
//! wire, through core's client.
//!
//! They need the `mining-control` grant. The wallet's own node admits them
//! on its owner's leg, where the wallet holds every grant. A node address a
//! person typed is a view listener, which refuses them, and the page says
//! so in the node's words (`daemon_rpc::said`). The miner pays the node's
//! own `mine-address`, set when the node starts.

use serde::Serialize;
use shekyl_rpc_client::wire::node::{AutoWord, MineStart, Threads};
use shekyl_rpc_client::wire::{MineStatus, MineStop};
use shekyl_rpc_types::kinds::{Count, NoParams};
use shekyl_units::AtomicUnitsString;
use tauri::State;

use crate::daemon_rpc;
use crate::state::AppState;

/// The node's miner (`mine_status`). The miner pays the node's own
/// `mine-address`, set when the node starts: `mine_start` takes no address
/// (shekyl-core IG-Q77, IG-F23). Each figure of the posted job is `null`
/// while no job is posted, and `cause` says why.
#[derive(Debug, Serialize)]
pub struct MiningStatus {
    pub active: bool,
    /// Hashes per second over the node's last window; `null` before a
    /// window has completed.
    pub hash_rate: Option<f64>,
    pub threads: u64,
    /// The address the miner pays; `null` when the node has none set.
    pub address: Option<String>,
    /// The chain's block spacing, seconds.
    pub block_spacing: u64,
    pub height: Option<u64>,
    pub block_reward: Option<AtomicUnitsString>,
    /// The posted job's difficulty, as a decimal string (a 128-bit value).
    pub difficulty: Option<String>,
    /// Why no job is posted, in the node's words.
    pub cause: Option<String>,
    /// Whose address the miner pays, as far as this wallet can tell.
    pub payout: Payout,
    /// Whether Start mining here pays the open wallet. `false` on a node
    /// address a person typed: its operator's address is paid, set or not.
    pub start_pays_the_open_wallet: bool,
}

/// Whose address the node's miner pays. The page says which, so a person
/// never reads "the open wallet" where the rewards go elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Payout {
    /// The open wallet's.
    OpenWallet,
    /// Another address, set on the wallet's own node for a wallet that is
    /// not open (Start mining moves it to the open wallet).
    AnotherWallet,
    /// The address a node's operator set, on a node address a person
    /// typed: Start mining does not change it.
    SetByTheNode,
    /// None, on the wallet's own node: it posts no job, and Start mining
    /// gives it the open wallet's.
    Nothing,
    /// None, on a node address a person typed: its operator sets one, and
    /// Start mining does not.
    NoneOnTheNode,
}

impl Payout {
    /// Whether Start mining on this node pays the open wallet: on the
    /// wallet's own node, never on a node address a person typed.
    const fn start_pays_the_open_wallet(self) -> bool {
        match self {
            Self::OpenWallet | Self::AnotherWallet | Self::Nothing => true,
            Self::SetByTheNode | Self::NoneOnTheNode => false,
        }
    }
}

/// Whose address `address` is, for the page.
fn payout(address: Option<&str>, open_wallet: Option<&str>, typed_node: bool) -> Payout {
    match (address, typed_node) {
        (None, false) => Payout::Nothing,
        (None, true) => Payout::NoneOnTheNode,
        (Some(_), true) => Payout::SetByTheNode,
        (Some(paid), false) if Some(paid) == open_wallet => Payout::OpenWallet,
        (Some(_), false) => Payout::AnotherWallet,
    }
}

/// What a mining read or change is called when the node refuses it.
const MINING: &str = "mining";

#[tauri::command]
pub async fn get_mining_status(
    state: State<'_, AppState>,
    node: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<MiningStatus, String> {
    let ms = daemon_rpc::node_call::<MineStatus>(&state, &node, NoParams, MINING).await?;
    let open_wallet = {
        let eng = state.engine.lock().await;
        if eng.is_open() {
            eng.primary_address().await.ok()
        } else {
            None
        }
    };
    let typed_node = !state.targets_this_computer().await;
    let mut status = mining_status(ms);
    status.set_payout(payout(
        status.address.as_deref(),
        open_wallet.as_deref(),
        typed_node,
    ));
    Ok(status)
}

impl MiningStatus {
    fn set_payout(&mut self, payout: Payout) {
        self.payout = payout;
        self.start_pays_the_open_wallet = payout.start_pays_the_open_wallet();
    }
}

/// The miner's state from the node's `mine_status` reply.
fn mining_status(ms: shekyl_rpc_client::wire::node::MineStatus) -> MiningStatus {
    MiningStatus {
        active: ms.active,
        // Hashes counted over a window of whole seconds; no rate before
        // the first window completes.
        hash_rate: (ms.speed.window.0 > 0)
            .then(|| ms.speed.count.0 as f64 / ms.speed.window.0 as f64),
        threads: ms.threads.0,
        address: ms.address.0.map(|a| a.0),
        block_spacing: ms.block_spacing.0,
        height: ms.height.0.map(|h| h.0),
        block_reward: ms.reward.0,
        difficulty: ms.difficulty.0.map(|d| d.get().to_string()),
        cause: ms.cause.0.map(|c| c.message),
        payout: Payout::Nothing,
        start_pays_the_open_wallet: Payout::Nothing.start_pays_the_open_wallet(),
    }
}

/// Start the node's miner with `threads` threads, or the node's own
/// `mine-threads` when none is given.
///
/// On the wallet's own node the miner pays the open wallet's address. The
/// daemon reads `mine-address` only at its start (shekyl-core
/// `INPUT_GRAMMAR.md` IG-12), so a node running with another address, or
/// none, is stopped and started again with this one before the miner
/// starts: one transaction in the node's manager, against the wallet open
/// when it runs (`DaemonManager::start_mining`). A node address a person
/// typed keeps the address its operator set.
#[tauri::command]
pub async fn start_mining_cmd(
    state: State<'_, AppState>,
    node: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
    threads: Option<u64>,
) -> Result<bool, String> {
    let threads = threads.map_or(Threads::Auto(AutoWord::Auto), |n| Threads::Count(Count(n)));
    match state.node_address().await {
        None => node.start_mining(threads).await?,
        Some(address) => {
            daemon_rpc::call::<shekyl_rpc_client::wire::MineStart>(
                &address,
                MineStart { threads },
                MINING,
            )
            .await?;
        }
    }
    Ok(true)
}

#[tauri::command]
pub async fn stop_mining_cmd(
    state: State<'_, AppState>,
    node: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<bool, String> {
    daemon_rpc::node_call::<MineStop>(&state, &node, NoParams, MINING).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Core's `mine_status` vector: a node with no mining address posts no
    /// job. The page gets no address, no job figures and no rate, and the
    /// node's own sentence for why.
    #[test]
    fn a_node_without_a_mining_address_says_why_and_shows_no_figures() {
        let ms = mining_status(crate::wire_vectors::reply("mine_status"));
        assert!(!ms.active);
        assert_eq!(ms.address, None);
        assert_eq!(ms.hash_rate, None, "a zero window is no rate, not 0 H/s");
        assert_eq!(ms.height, None);
        assert!(ms.block_reward.is_none() && ms.difficulty.is_none());
        assert_eq!(
            ms.cause.as_deref(),
            Some("no mine-address is set: this node posts no job")
        );
    }

    /// The rate is hashes over the window, and the job's 128-bit
    /// difficulty crosses as a decimal string.
    #[test]
    fn the_rate_is_hashes_over_the_window() {
        let ms: shekyl_rpc_client::wire::node::MineStatus =
            serde_json::from_value(serde_json::json!({
                "active": true,
                "speed": {"count": 6000, "window": 5},
                "threads": 4,
                "address": "SKL1abc",
                "block_spacing": 120,
                "height": 10,
                "reward": "5000000000",
                "difficulty": "0x2a3f1c",
                "cause": null,
                "txpool_retired": false
            }))
            .expect("a 4.0 mine_status reply");
        let m = mining_status(ms);
        assert_eq!(m.hash_rate, Some(1200.0));
        assert_eq!(m.address.as_deref(), Some("SKL1abc"));
        assert_eq!(m.difficulty.as_deref(), Some("2768668"));
    }

    /// The page is told whose address the miner pays: the open wallet's,
    /// another wallet's on the wallet's own node, or one a typed node's
    /// operator set, which Start mining does not change. Red edit: report
    /// `OpenWallet` for any address (a typed node's rewards would read as
    /// the open wallet's).
    #[test]
    fn the_page_is_told_whose_address_the_miner_pays() {
        assert_eq!(payout(None, Some("SKL1a"), false), Payout::Nothing);
        // A typed node with no address is not the wallet's to give one:
        // Start mining there pays whatever its operator sets. Red edit:
        // `(None, _) => Payout::Nothing`.
        assert_eq!(payout(None, Some("SKL1a"), true), Payout::NoneOnTheNode);
        for (payout, pays_open) in [
            (Payout::OpenWallet, true),
            (Payout::AnotherWallet, true),
            (Payout::Nothing, true),
            (Payout::SetByTheNode, false),
            (Payout::NoneOnTheNode, false),
        ] {
            assert_eq!(payout.start_pays_the_open_wallet(), pays_open, "{payout:?}");
        }
        assert_eq!(
            serde_json::to_value(Payout::NoneOnTheNode).unwrap(),
            "none-on-the-node"
        );
        assert_eq!(
            payout(Some("SKL1a"), Some("SKL1a"), false),
            Payout::OpenWallet
        );
        assert_eq!(
            payout(Some("SKL1b"), Some("SKL1a"), false),
            Payout::AnotherWallet
        );
        assert_eq!(payout(Some("SKL1b"), None, false), Payout::AnotherWallet);
        assert_eq!(
            payout(Some("SKL1a"), Some("SKL1a"), true),
            Payout::SetByTheNode
        );
        assert_eq!(
            serde_json::to_value(Payout::SetByTheNode).unwrap(),
            "set-by-the-node"
        );
    }
}
