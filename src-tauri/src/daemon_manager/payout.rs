// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

//! What the wallet's node's miner pays, and the starts again that its
//! settings need (`docs/design/GUI_SYNC_LANE.md` GS-2, GS-4).
//!
//! The daemon reads `mine-address` and its listener settings only at its
//! start, so each change here is a stop and a start of the wallet's own
//! node. Every one of them runs under the manager's lifecycle lock, start
//! to finish, miner included, so two of them never interleave and a
//! payout is always reconciled against the wallet open *now*, not the one
//! open when the reconciling was asked for.

use std::sync::Arc;

use shekyl_rpc_client::wire::node::{MineStart, Threads};
use shekyl_rpc_client::wire::{MineStatus, MineStop};
use shekyl_rpc_types::kinds::{Count, NoParams};

use super::{found_words, DaemonManager, NodeState};

impl DaemonManager {
    /// Have the wallet's node pay its miner to `address` (Start mining).
    /// A node running with another address, or none, is stopped and
    /// started again with this one. Returns once the node is ready, or
    /// with the reason it is not.
    pub async fn mine_to(self: &Arc<Self>, address: &str) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        let paid = self.mine_to_locked(address).await;
        if paid.is_ok() {
            self.inner.write().await.payout_fault = None;
        }
        paid
    }

    async fn mine_to_locked(self: &Arc<Self>, address: &str) -> Result<(), String> {
        {
            let inner = self.inner.read().await;
            match &inner.state {
                NodeState::Running if inner.mine_address.as_deref() == Some(address) => {
                    return Ok(())
                }
                NodeState::Running => {}
                NodeState::Found => return Err(found_words(inner.network)),
                NodeState::Service(view) => return Err(shekyl_rpc_client::service_refusal(*view)),
                NodeState::Waiting(words)
                | NodeState::Refused(words)
                | NodeState::Unavailable(words) => return Err(words.clone()),
                NodeState::Idle
                | NodeState::Starting
                | NodeState::Stopping
                | NodeState::Failed(_) => {}
            }
        }
        self.stop_own_locked().await;
        self.inner.write().await.mine_address = Some(address.to_owned());
        let node = self.start_locked().await;
        self.ready_or_words(node).await
    }

    /// A wallet was opened (`Some` with its address) or closed (`None`).
    /// The open wallet is kept here, and the payout is reconciled against
    /// it beside the open, never inside it ([`Self::reconcile_payout`]).
    pub(crate) async fn wallet_is(self: &Arc<Self>, address: Option<String>) {
        let opened = address.is_some();
        self.inner.write().await.open_wallet = address;
        if opened {
            let manager = Arc::clone(self);
            tokio::spawn(async move { manager.reconcile_payout().await });
        }
    }

    /// [`Self::wallet_is`], waiting for the reconciling it starts: for a
    /// test that reads what the miner pays once it is done.
    #[cfg(test)]
    pub(crate) async fn wallet_is_reconciled(self: &Arc<Self>, address: &str) {
        self.inner.write().await.open_wallet = Some(address.to_owned());
        self.reconcile_payout().await;
    }

    /// The fault the last reconciling left, for a test.
    #[cfg(test)]
    pub(crate) async fn payout_fault(&self) -> Option<String> {
        self.inner.read().await.payout_fault.clone()
    }

    /// Move a running miner to the open wallet's address (Rick,
    /// 2026-11-17): only while the miner is running and pays another
    /// address. The node is started again with the open wallet's address and
    /// the miner goes on with the threads it had. A node that is not mining
    /// is left alone; [`Self::mine_to`] gives it the address when mining is
    /// started.
    ///
    /// Under the lifecycle lock, against the wallet open when it runs: a
    /// reconciling asked for by an earlier open finds the later wallet, and
    /// one that waited behind another start finds the miner that start
    /// left running. If the move fails, the miner is stopped, so it never
    /// goes on paying a wallet that is not open, and the node's status says
    /// so ([`super::DaemonStatus::payout_fault`]).
    async fn reconcile_payout(self: &Arc<Self>) {
        let _lifecycle = self.lifecycle.lock().await;
        let (open, paid) = {
            let inner = self.inner.read().await;
            (inner.open_wallet.clone(), inner.mine_address.clone())
        };
        let Some(open) = open else { return };
        if paid.is_none_or(|paid| paid == open) {
            return;
        }
        let moved = async {
            let Some(threads) = self.running_miner().await? else {
                return Ok(());
            };
            self.mine_to_locked(&open).await?;
            self.start_miner(threads).await
        }
        .await;
        let fault = match moved {
            Ok(()) => None,
            Err(e) => {
                tracing::warn!("the miner could not be moved to the open wallet's address: {e}");
                let stopped = self
                    .own_call::<MineStop>(NoParams, "the miner's stop")
                    .await
                    .is_ok();
                Some(if stopped {
                    format!(
                        "Mining was stopped: it could not be moved to the open wallet's \
                         address. {e}"
                    )
                } else {
                    format!(
                        "Mining could not be moved to the open wallet's address, and the \
                         miner may still be paying the wallet open before. {e}"
                    )
                })
            }
        };
        self.inner.write().await.payout_fault = fault;
    }

    /// The threads of the wallet's own node's miner while it runs. `None`
    /// when it does not, and when the wallet has no node of its own ready:
    /// nothing of its is mining then.
    async fn running_miner(&self) -> Result<Option<u64>, String> {
        if self.own_target().await.is_err() {
            return Ok(None);
        }
        let miner = self
            .own_call::<MineStatus>(NoParams, "the node's miner")
            .await?;
        Ok(miner.active.then_some(miner.threads.0))
    }

    /// Start the miner of the node just started again, with the threads it
    /// had before.
    async fn start_miner(&self, threads: u64) -> Result<(), String> {
        let threads = Threads::Count(Count(threads));
        self.own_call::<shekyl_rpc_client::wire::MineStart>(
            MineStart { threads },
            "the miner's restart",
        )
        .await
        .map(drop)
    }

    /// Accept inbound clearnet connections or not (`--clearnet-listen`):
    /// kept in the node's settings and passed at its next start, so the
    /// node is started again with it. Off is the wallet's default (Rick,
    /// 2026-11-17); on, the node listens on its network's peer port.
    pub async fn set_clearnet_inbound(self: &Arc<Self>, on: bool) -> Result<(), String> {
        let mut config = self.config().await;
        if config.clearnet_inbound == on {
            return Ok(());
        }
        tracing::info!(
            on,
            "the node's inbound clearnet listener changes; the node is started again"
        );
        config.clearnet_inbound = on;
        self.update_config(config).await?;
        self.relaunch_for_settings().await
    }

    /// The node's settings changed (the inbound clearnet listener, a Tor
    /// fix taken or put back): the daemon reads them only at its start, so
    /// a node this wallet started is stopped and started again with them,
    /// and a miner that was running goes on, all under the lifecycle lock.
    /// A node that had failed to start is started. A node of this account
    /// the wallet found is left alone: it is not the wallet's, and the
    /// settings are for the wallet's own.
    pub async fn relaunch_for_settings(self: &Arc<Self>) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        {
            let inner = self.inner.read().await;
            match &inner.state {
                NodeState::Found => return Err(found_words(inner.network)),
                NodeState::Refused(words) | NodeState::Unavailable(words) => {
                    return Err(words.clone())
                }
                _ => {}
            }
        }
        let miner = self.running_miner().await?;
        self.stop_own_locked().await;
        let node = self.start_locked().await;
        self.ready_or_words(node).await?;
        match miner {
            Some(threads) => self.start_miner(threads).await,
            None => Ok(()),
        }
    }
}
