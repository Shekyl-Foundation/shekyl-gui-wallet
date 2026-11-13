// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Consensus constants the pages name, from core, read without the node.
//!
//! A constant is the same on every node and known to this build, so a page
//! that names one does not wait on the node or go blank when it is down. A
//! figure the node reports (its block spacing) stays on the chain read.

use serde::Serialize;

/// The constants a page names.
#[derive(Debug, Serialize)]
pub struct ChainConstants {
    /// Blocks a mined reward waits before it can be spent
    /// (`shekyl_consensus::COINBASE_LOCK_WINDOW`).
    pub coinbase_lock_window: u64,
}

fn chain_constants() -> ChainConstants {
    ChainConstants {
        coinbase_lock_window: u64::try_from(shekyl_consensus::COINBASE_LOCK_WINDOW)
            .expect("the coinbase lock window fits u64"),
    }
}

#[tauri::command]
pub fn get_chain_constants() -> ChainConstants {
    chain_constants()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window is core's constant, served with no node.
    #[test]
    fn the_lock_window_is_core_s_constant() {
        let v = serde_json::to_value(chain_constants()).unwrap();
        assert_eq!(
            v["coinbase_lock_window"],
            shekyl_consensus::COINBASE_LOCK_WINDOW as u64
        );
    }
}
