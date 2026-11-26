// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

//! The wallet engine's way to "this computer", and the one latch for a
//! failed peer check (`docs/design/GUI_SYNC_LANE.md` GS-2; Rick,
//! 2026-11-18).
//!
//! The engine dials "this computer" itself, with a client of its own, and
//! the wallet's own calls dial it from the manager. A failed peer check on
//! either is the same fact: something that is not this account's node
//! answers at the node's local address. It is kept in one place, the
//! manager's `refused` state, so the notice says it wherever it was met and
//! neither side dials that address again until a person starts the node
//! again. Only the failed check is latched here: a node found, a node that
//! failed, and a service do not stand in the engine's way (no engine gate).

use shekyl_engine_core::Network;
#[cfg(test)]
use shekyl_rpc_transport::local::LocalRpc;
use shekyl_rpc_transport::{DaemonTarget, DaemonTransport};

use super::{DaemonManager, NodeState};

impl DaemonManager {
    /// A failed peer check at the node's local address, met by the
    /// wallet's own call or by its engine: the state is `refused`, in the
    /// check's own words. The wallet's node, if it started one, is not
    /// killed: it is still the wallet's, by its owning connection.
    pub(crate) async fn refuse(&self, words: String) {
        tracing::error!("the node's local address failed its check: {words}");
        self.set_state(NodeState::Refused(words)).await;
    }

    /// The failed check's words, while one is latched.
    pub(crate) async fn refusal(&self) -> Option<String> {
        match &self.inner.read().await.state {
            NodeState::Refused(words) => Some(words.clone()),
            _ => None,
        }
    }

    /// The engine's transport to "this computer" on `network`, named by
    /// the network alone as core defines it. Refused, without a dial,
    /// while a failed peer check is latched.
    pub(crate) async fn engine_transport(
        &self,
        network: Network,
    ) -> Result<DaemonTransport, String> {
        if let Some(words) = self.refusal().await {
            return Err(words);
        }
        #[cfg(test)]
        if let Some(target) = self.test_target(network) {
            return Ok(LocalRpc::new(target).into());
        }
        DaemonTarget::ThisComputer {
            network,
            account: shekyl_rpc_transport::ChannelAccount::default(),
        }
            .connect(None)
            .await
            .map_err(|e| format!("daemon unreachable: {e}"))
    }
}
