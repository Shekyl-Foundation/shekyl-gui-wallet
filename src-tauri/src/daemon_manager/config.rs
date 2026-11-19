// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The wallet's node settings: what a person chose for the node the wallet
//! starts, kept as `daemon.json` in the GUI's settings directory
//! ([`crate::account_root::settings_dir`]), and read back at the next
//! start.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The node settings a person chose.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub data_dir: Option<String>,
    /// The Tor fix a person took; none is the node's own defaults with the
    /// bundled Tor (`crate::tor_fix`).
    #[serde(default)]
    pub tor_fix: Option<crate::tor_fix::TorFix>,
    /// Whether the node accepts inbound connections over clearnet. Off by
    /// default: the wallet's node dials out and takes none (Rick,
    /// 2026-11-17). On, it listens on its network's peer port.
    #[serde(default)]
    pub clearnet_inbound: bool,
}

/// What the wallet says when the account has no home folder: its node has
/// nowhere to keep its settings, so it is not started and nothing is saved.
pub(crate) const NO_SETTINGS_DIR: &str =
    "This account has no home folder, so the wallet's node has nowhere to keep its settings and is not started.";

impl DaemonConfig {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("daemon.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), String> {
        let path = config_dir.join("daemon.json");
        std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}
