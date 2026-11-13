// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{engine_session, gui_config};
use shekyl_engine_core::Network;

/// The networks a wallet can be on: the wallet contract's three
/// (`wallet_rpc.yaml`, `MAINNET` / `TESTNET` / `STAGENET`). Core's
/// [`Network`] also has regtest, whose row alone serves the test levers
/// (NI-R6) and which no wallet contract names, so this type stays the
/// contract's and maps to core's in [`NetworkType::network`], the one
/// mapping. Every fact about a network (its name, its ports, its data
/// subdirectory) is read from core's row through it; the GUI holds none.
///
/// The default is mainnet (Rick, 2026-11-13).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    #[default]
    Mainnet,
    Testnet,
    Stagenet,
}

impl NetworkType {
    /// Every wallet network.
    pub const ALL: [Self; 3] = [Self::Mainnet, Self::Testnet, Self::Stagenet];

    /// Core's network: its row holds every fact about it.
    pub const fn network(self) -> Network {
        match self {
            Self::Mainnet => Network::Mainnet,
            Self::Testnet => Network::Testnet,
            Self::Stagenet => Network::Stagenet,
        }
    }

    /// The daemon's RPC port on this network, from core's row.
    pub const fn rpc_port(self) -> u16 {
        self.network().ports().rpc
    }

    /// The network's name, from core's row (`mainnet`, ...).
    pub const fn as_str(self) -> &'static str {
        self.network().name()
    }

    /// The network a name names, among the wallet's networks.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.as_str() == name)
    }

    /// The URL of the node on this computer at the network's RPC port.
    pub fn local_url(self) -> String {
        format!("http://127.0.0.1:{}/json_rpc", self.rpc_port())
    }
}

pub fn default_wallet_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".shekyl")
            .join("wallets")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shekyl")
            .join("wallets")
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shekyl")
            .join("wallets")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".shekyl")
            .join("wallets")
    }
}

pub struct AppState {
    // Daemon
    pub daemon_url: RwLock<String>,
    pub network: RwLock<NetworkType>,

    // Wallet directory / open metadata
    pub wallet_dir: RwLock<PathBuf>,
    /// `Some(path)` when the persisted wallet-dir override was
    /// unreachable at startup and we fell back to the platform default.
    /// Surfaced via `get_wallet_dir` so the UI can warn the user;
    /// cleared by `set_wallet_dir` / `reset_wallet_dir` once the user
    /// picks a working location.
    pub wallet_dir_warning: RwLock<Option<PathBuf>>,
    pub wallet_open: RwLock<bool>,
    pub wallet_name: RwLock<Option<String>>,
    /// Pure-Rust Engine session — the sole wallet backend.
    pub engine: tokio::sync::Mutex<engine_session::EngineSession>,
}

impl AppState {
    pub fn new() -> Self {
        let net = NetworkType::default();
        let resolved = gui_config::resolve_wallet_dir(default_wallet_dir());
        Self {
            daemon_url: RwLock::new(net.local_url()),
            network: RwLock::new(net),
            wallet_dir: RwLock::new(resolved.dir),
            wallet_dir_warning: RwLock::new(resolved.fallback_from),
            wallet_open: RwLock::new(false),
            wallet_name: RwLock::new(None),
            engine: tokio::sync::Mutex::new(engine_session::EngineSession::new()),
        }
    }

    /// HTTP base URL for Engine daemon client (no `/json_rpc` suffix).
    pub async fn daemon_http_base(&self) -> String {
        self.base_url().await
    }

    pub async fn url(&self) -> String {
        self.daemon_url.read().await.clone()
    }

    /// Base URL without the `/json_rpc` suffix: the Engine daemon-client
    /// base.
    ///
    /// Trailing slashes are stripped *before* the `/json_rpc` suffix so a
    /// configured URL like `http://host:port/json_rpc/` collapses to
    /// `http://host:port` rather than leaving a stray `json_rpc` behind.
    pub async fn base_url(&self) -> String {
        let url = self.daemon_url.read().await.clone();
        strip_json_rpc(&url).to_owned()
    }
}

/// Strip a trailing `/json_rpc` (with any surrounding slashes) from a daemon
/// URL, leaving the plain HTTP base. Slashes are trimmed on both sides of the
/// suffix so `http://host/json_rpc`, `http://host/json_rpc/`, and
/// `http://host/` all collapse to `http://host`.
fn strip_json_rpc(url: &str) -> &str {
    let trimmed = url.trim_end_matches('/');
    trimmed
        .strip_suffix("/json_rpc")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
}
