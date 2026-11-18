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

/// Whether the wallet engine may dial the node on this computer. It may
/// only while the node there is the one this wallet started and that node
/// has said it is ready: its ready line follows its view listener's bind,
/// so before it the port may be anyone's. Not before that node starts, not
/// while a node of this account that the wallet did not start answers
/// there, not after a failed check or a failed start. Until the engine's
/// calls move to the owner's leg (shekyl-core `RPC_CHANNEL.md` RT-W10 step
/// 5), the engine never uses a listener on a node the wallet did not launch
/// (Rick, 2026-11-17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// No daemon manager: nothing of the wallet's runs here to wait for,
    /// and the configured URL is used as it is.
    Unmanaged,
    /// The wallet's own node is ready, and this is its view listener's
    /// base URL: the engine's address comes with the node that said it is
    /// ready, never from a setting published ahead of it.
    Open(String),
    /// The wallet's own node is starting: a caller waits for its ready
    /// line or its end, either of which comes.
    Starting,
    /// Closed, and why, in words.
    Closed(String),
}

/// The gate, written by the daemon manager and read by the engine's way to
/// the node.
#[derive(Clone)]
pub struct NodeGate(std::sync::Arc<tokio::sync::watch::Sender<Gate>>);

impl Default for NodeGate {
    /// Open: with no daemon manager there is no node of the wallet's to
    /// wait for.
    fn default() -> Self {
        Self(std::sync::Arc::new(
            tokio::sync::watch::channel(Gate::Unmanaged).0,
        ))
    }
}

impl NodeGate {
    pub(crate) fn set(&self, gate: Gate) {
        self.0.send_replace(gate);
    }

    /// The ready node's base URL once the gate is open (`None` with no
    /// daemon manager), waiting while the node is starting; otherwise why
    /// it is closed.
    pub(crate) async fn passable(&self) -> Result<Option<String>, String> {
        let mut gate = self.0.subscribe();
        loop {
            let now = gate.borrow_and_update().clone();
            match now {
                Gate::Unmanaged => return Ok(None),
                Gate::Open(base) => return Ok(Some(base)),
                Gate::Closed(words) => return Err(words),
                Gate::Starting => {}
            }
            // The sender lives as long as `self`, so the change comes.
            if gate.changed().await.is_err() {
                return Err("The wallet's node is gone.".to_owned());
            }
        }
    }
}

pub struct AppState {
    // Daemon
    pub daemon_url: RwLock<String>,
    /// Closed while the node on this computer is not the wallet's own.
    pub node_gate: NodeGate,
    /// Whether `daemon_url` is a node address a person typed. Otherwise it
    /// is the node on this computer, the wallet's own.
    pub typed_node: RwLock<bool>,
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
    /// The app's state on mainnet, for a test.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::on(NetworkType::default())
    }

    /// The app's state on `net`, the network its node starts on.
    pub fn on(net: NetworkType) -> Self {
        let resolved = gui_config::resolve_wallet_dir(default_wallet_dir());
        Self {
            daemon_url: RwLock::new(net.local_url()),
            node_gate: NodeGate::default(),
            typed_node: RwLock::new(false),
            network: RwLock::new(net),
            wallet_dir: RwLock::new(resolved.dir),
            wallet_dir_warning: RwLock::new(resolved.fallback_from),
            wallet_open: RwLock::new(false),
            wallet_name: RwLock::new(None),
            engine: tokio::sync::Mutex::new(engine_session::EngineSession::new()),
        }
    }

    pub async fn url(&self) -> String {
        self.daemon_url.read().await.clone()
    }

    /// Whether the wallet is pointed at the node on this computer, the
    /// default, and not at a node address a person typed. An address a
    /// person typed is theirs even when it names this computer: it is
    /// called as any explicit node is, over HTTP, with what it grants.
    pub async fn targets_this_computer(&self) -> bool {
        !*self.typed_node.read().await
    }

    /// The wallet engine's daemon base URL, without the `/json_rpc` suffix.
    ///
    /// Trailing slashes are stripped *before* the `/json_rpc` suffix so a
    /// configured URL like `http://host:port/json_rpc/` collapses to
    /// `http://host:port` rather than leaving a stray `json_rpc` behind.
    ///
    /// # Errors
    ///
    /// The wallet is pointed at this computer and the node there is not the
    /// wallet's own ([`NodeGate`]): the words say which.
    pub async fn engine_base(&self) -> Result<String, String> {
        if self.targets_this_computer().await {
            if let Some(base) = self.node_gate.passable().await? {
                return Ok(base);
            }
        }
        let url = self.daemon_url.read().await.clone();
        Ok(strip_json_rpc(&url).to_owned())
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
