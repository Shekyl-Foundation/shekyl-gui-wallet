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

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{engine_session, gui_config};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    #[default]
    Mainnet,
    Testnet,
    Stagenet,
}

impl NetworkType {
    pub fn default_rpc_port(self) -> u16 {
        match self {
            Self::Mainnet => 11029,
            Self::Testnet => 12029,
            Self::Stagenet => 13029,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
            Self::Stagenet => "stagenet",
        }
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
    pub http: Client,

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
            daemon_url: RwLock::new(format!(
                "http://127.0.0.1:{}/json_rpc",
                net.default_rpc_port()
            )),
            network: RwLock::new(net),
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to create HTTP client"),
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

    /// Base URL without the `/json_rpc` suffix, for plain HTTP endpoints
    /// like `/mining_status`, `/start_mining`, `/stop_mining`, and as the
    /// Engine daemon-client base.
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
