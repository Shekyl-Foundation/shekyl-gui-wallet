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

use crate::wallet_bridge;

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

    // Wallet (direct FFI via shekyl-engine-rpc)
    pub wallet_dir: RwLock<PathBuf>,
    pub wallet_open: RwLock<bool>,
    pub wallet_name: RwLock<Option<String>>,
    pub wallet: wallet_bridge::WalletHandle,
}

impl AppState {
    pub fn new() -> Self {
        let net = NetworkType::default();
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
            wallet_dir: RwLock::new(default_wallet_dir()),
            wallet_open: RwLock::new(false),
            wallet_name: RwLock::new(None),
            wallet: wallet_bridge::new_handle(),
        }
    }

    pub async fn url(&self) -> String {
        self.daemon_url.read().await.clone()
    }

    /// Base URL without the `/json_rpc` suffix, for plain HTTP endpoints
    /// like `/mining_status`, `/start_mining`, `/stop_mining`.
    pub async fn base_url(&self) -> String {
        let url = self.daemon_url.read().await.clone();
        url.trim_end_matches("/json_rpc").to_string()
    }

    /// Daemon address for wallet2 init (host:port without http:// or /json_rpc).
    pub async fn daemon_address(&self) -> String {
        let url = self.daemon_url.read().await.clone();
        url.trim_end_matches("/json_rpc")
            .trim_end_matches('/')
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string()
    }
}
