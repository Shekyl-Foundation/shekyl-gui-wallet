// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The node's miner: `mine_status`, `mine_start` and `mine_stop` on the 4.0
//! wire, through core's client.
//!
//! They need the `mining-control` grant. The node the wallet starts holds
//! only the view grants for the wallet, so it refuses them, and the page
//! says so in the node's words (`daemon_rpc::said`). The miner pays the
//! node's own `mine-address`, set when the node starts.

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
}

/// What a mining read or change is called when the node refuses it.
const MINING: &str = "mining";

#[tauri::command]
pub async fn get_mining_status(state: State<'_, AppState>) -> Result<MiningStatus, String> {
    let url = state.url().await;
    let ms = daemon_rpc::call::<MineStatus>(&url, NoParams, MINING).await?;
    Ok(mining_status(ms))
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
    }
}

/// Start the node's miner with `threads` threads, or the node's own
/// `mine-threads` when none is given.
#[tauri::command]
pub async fn start_mining_cmd(
    state: State<'_, AppState>,
    threads: Option<u64>,
) -> Result<bool, String> {
    let url = state.url().await;
    let threads = threads.map_or(Threads::Auto(AutoWord::Auto), |n| Threads::Count(Count(n)));
    daemon_rpc::call::<shekyl_rpc_client::wire::MineStart>(&url, MineStart { threads }, MINING)
        .await?;
    Ok(true)
}

#[tauri::command]
pub async fn stop_mining_cmd(state: State<'_, AppState>) -> Result<bool, String> {
    let url = state.url().await;
    daemon_rpc::call::<MineStop>(&url, NoParams, MINING).await?;
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
}
