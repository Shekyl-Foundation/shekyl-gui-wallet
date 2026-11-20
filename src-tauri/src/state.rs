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

use crate::account_root::AccountRoot;
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

    /// The network's name, from core's row (`mainnet`, ...).
    pub const fn as_str(self) -> &'static str {
        self.network().name()
    }

    /// The network a name names, among the wallet's networks.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.as_str() == name)
    }
}

pub struct AppState {
    // Daemon
    /// The node address a person typed, which the wallet then uses, local
    /// or not: their choice (Rick, 2026-11-18). `None`: "this computer",
    /// the one target (core `RPC_CHANNEL.md` RT-15), reached over its
    /// owner's leg.
    pub node_address: RwLock<Option<String>>,
    pub network: RwLock<NetworkType>,
    /// Where the GUI's settings are kept: this account's file, named once
    /// at the start; a test's state names its own or none.
    pub settings: gui_config::Store,

    // Wallet directory / open metadata
    /// This account's per-account root as the app's start left it: the
    /// default wallet directory, or why there is none.
    pub root: AccountRoot,
    /// The wallet directory the person chose, taken as given on every
    /// network; `None` when they chose none. Read the wallet directory
    /// through [`AppState::wallet_dir`], which gives the network's default
    /// when there is no choice.
    pub wallet_dir: RwLock<Option<PathBuf>>,
    /// `Some(path)` when the persisted wallet-dir override was
    /// unreachable at startup and we fell back to the platform default.
    /// Surfaced via `get_wallet_dir` so the UI can warn the user;
    /// cleared by `set_wallet_dir` / `reset_wallet_dir` once the user
    /// picks a working location.
    pub wallet_dir_warning: RwLock<Option<PathBuf>>,
    pub wallet_open: RwLock<bool>,
    pub wallet_name: RwLock<Option<String>>,
    /// Pure-Rust Engine session — the sole wallet backend.
    ///
    /// Its mutex is also the session guard. Whatever opens or closes a
    /// wallet, and whatever changes the network or the node address, holds
    /// it from its first read of them to its last write, and tells the
    /// node's manager of the wallet inside it. So a wallet is never opened
    /// on a network or a node that a change is replacing, and the manager
    /// hears of opens and closes in the order they happened. Lock order:
    /// this guard, then the fields above, then the manager.
    pub engine: tokio::sync::Mutex<engine_session::EngineSession>,
}

impl AppState {
    /// The app's state on mainnet, for a test: no root, so nothing is read
    /// from or made in the home of the account the tests run under.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::on(
            NetworkType::default(),
            gui_config::Store::none(),
            AccountRoot::NoHome,
        )
    }

    /// The app's state on `net`, the network its node starts on, with its
    /// settings kept in `settings` and its default wallet directory read
    /// off `root`.
    pub fn on(net: NetworkType, settings: gui_config::Store, root: AccountRoot) -> Self {
        let resolved = gui_config::resolve_wallet_dir(settings.load());
        Self {
            node_address: RwLock::new(None),
            network: RwLock::new(net),
            settings,
            root,
            wallet_dir: RwLock::new(resolved.choice),
            wallet_dir_warning: RwLock::new(resolved.fallback_from),
            wallet_open: RwLock::new(false),
            wallet_name: RwLock::new(None),
            engine: tokio::sync::Mutex::new(engine_session::EngineSession::new()),
        }
    }

    /// The wallet directory: the person's choice, else the default of the
    /// network the wallet is on now (`wallets/<network>` in the root), else
    /// the words that say why there is none. The default is read here, not
    /// kept, because the network changes while the app runs.
    pub async fn wallet_dir(&self) -> Result<PathBuf, String> {
        match self.wallet_dir.read().await.clone() {
            Some(dir) => Ok(dir),
            None => {
                let network = self.network.read().await.network();
                self.root.default_wallet_dir(network)
            }
        }
    }

    /// The node address a person typed, if any.
    pub async fn node_address(&self) -> Option<String> {
        self.node_address.read().await.clone()
    }

    /// Whether the wallet is pointed at the node on this computer, the
    /// default, and not at a node address a person typed. An address a
    /// person typed is theirs even when it names this computer: it is
    /// called as any explicit node is, over HTTP, with what it grants.
    pub async fn targets_this_computer(&self) -> bool {
        self.node_address.read().await.is_none()
    }

    /// The node the wallet engine reads, one target with no fallback (core
    /// `RPC_CHANNEL.md` RT-15; Rick, 2026-11-18): "this computer", named by
    /// its network alone as core defines it, or the node address a person
    /// typed.
    pub async fn engine_target(&self) -> shekyl_rpc_transport::DaemonTarget {
        match self.node_address().await {
            Some(address) => {
                shekyl_rpc_transport::DaemonTarget::address(strip_json_rpc(&address).to_owned())
            }
            None => shekyl_rpc_transport::DaemonTarget::ThisComputer {
                network: self.network.read().await.network(),
            },
        }
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

#[cfg(test)]
mod wallet_dir_tests {
    use super::*;

    use crate::account_root::NO_WALLET_DIR;

    /// With no home and no choice, a command that needs the wallet
    /// directory refuses in words.
    #[tokio::test]
    async fn no_wallet_dir_refuses_in_words() {
        let state = AppState::new();
        assert_eq!(*state.wallet_dir.read().await, None);
        assert_eq!(state.wallet_dir().await, Err(NO_WALLET_DIR.to_owned()));
    }

    /// A root the start could not make gives no default wallet directory:
    /// a command that needs one says which folder failed and why, and
    /// nothing makes the root without its protections. Red edit: a
    /// default read off the root's path whatever the start left (the old
    /// `default_wallet_dir()`) fails here.
    #[tokio::test]
    async fn a_root_not_made_gives_no_wallet_dir_and_says_why() {
        let root =
            std::env::temp_dir().join(format!("shekyl-gui-state-root-{}", std::process::id()));
        let state = AppState::on(
            NetworkType::Testnet,
            gui_config::Store::none(),
            AccountRoot::not_made_at(root.clone()),
        );
        assert_eq!(*state.wallet_dir.read().await, None);
        let words = state.wallet_dir().await.unwrap_err();
        assert!(words.contains(&root.display().to_string()), "{words}");
        assert!(!root.exists(), "nothing made the root");
    }

    /// Each network's wallets are in their own folder, read off the network
    /// the wallet is on when it is needed (Rick, 2026-11-20), and a folder
    /// the person chose is used on every network as given. Red edit: the
    /// default fixed at the start (the old `AppState::on`) keeps a switched
    /// wallet in the first network's folder.
    #[tokio::test]
    async fn the_default_wallet_dir_follows_the_network_and_a_choice_does_not() {
        let root = std::path::PathBuf::from("root");
        let state = AppState::on(
            NetworkType::Mainnet,
            gui_config::Store::none(),
            AccountRoot::Ready(root.clone()),
        );
        assert_eq!(
            state.wallet_dir().await,
            Ok(root.join("wallets").join("mainnet"))
        );
        *state.network.write().await = NetworkType::Testnet;
        assert_eq!(
            state.wallet_dir().await,
            Ok(root.join("wallets").join("testnet"))
        );
        let chosen = std::path::PathBuf::from("chosen");
        *state.wallet_dir.write().await = Some(chosen.clone());
        assert_eq!(state.wallet_dir().await, Ok(chosen.clone()));
        *state.network.write().await = NetworkType::Stagenet;
        assert_eq!(state.wallet_dir().await, Ok(chosen));
    }
}
