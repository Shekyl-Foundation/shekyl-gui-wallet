// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

//! Replacing a node of this account the wallet found running, and the
//! waits that go with it: for that node to be gone, and for the data
//! directory it held (`docs/design/GUI_SYNC_LANE.md` GS-2; Rick,
//! 2026-11-17). The wallet did not start that node, so it holds no
//! connection whose end would say it is gone: it looks, it waits, and a
//! person can end the wait. It never kills that node.

use std::sync::Arc;
use std::time::Duration;

use shekyl_rpc_client::wire::DaemonStop;
use shekyl_rpc_transport::local::LocalFault;
use shekyl_rpc_types::kinds::NoParams;

use super::{DaemonManager, NodeState, DATA_DIR_LOCKED};
use crate::state::NetworkType;

/// Spacing of the looks at a found node's rendezvous while it stops. This
/// wallet did not start that node and holds no connection whose end would
/// say it is gone, so it looks.
const FOUND_NODE_POLL: Duration = Duration::from_millis(500);

/// The first wait before the wallet's node is started again while another
/// process holds its data directory. Each further wait is twice the last,
/// up to [`HELD_DIR_LONGEST_WAIT`] (Rick, 2026-11-17).
const HELD_DIR_FIRST_WAIT: Duration = Duration::from_millis(500);

/// The longest wait between two such starts.
const HELD_DIR_LONGEST_WAIT: Duration = Duration::from_secs(4);

/// The wait after one of `waited`.
fn next_held_dir_wait(waited: Duration) -> Duration {
    waited.saturating_mul(2).min(HELD_DIR_LONGEST_WAIT)
}

/// The directory a `config.data-dir.locked` refusal names, read out of the
/// daemon's sentence (shekyl-core `shekyl-options`, `DataDirLocked`:
/// "another daemon is using the data directory <path>; stop it, …"). The
/// daemon chose the directory when the wallet's settings name none, so its
/// own sentence is where the wallet reads it.
fn held_directory(refusal: &str) -> Option<&str> {
    const BEFORE: &str = "the data directory ";
    const AFTER: &str = "; stop it";
    let rest = &refusal[refusal.find(BEFORE)? + BEFORE.len()..];
    let dir = &rest[..rest.find(AFTER)?];
    (!dir.is_empty()).then_some(dir)
}

/// What the notice says while the wallet waits for a held data directory.
fn waiting_words(refusal: &str) -> String {
    match held_directory(refusal) {
        Some(dir) => format!(
            "The node you stopped still holds the data directory {dir}. The wallet's node \
             starts when it lets go."
        ),
        None => "The node you stopped still holds the data directory. The wallet's node \
                 starts when it lets go."
            .to_owned(),
    }
}

/// What is said once a person has ended that wait.
fn left_waiting_words(refusal: &str) -> String {
    match held_directory(refusal) {
        Some(dir) => format!(
            "The wallet's node is not started: another program still holds the data \
             directory {dir}."
        ),
        None => "The wallet's node is not started: another program still holds the data \
                 directory."
            .to_owned(),
    }
}

/// What is said once a person has ended the wait for a found node to stop.
fn still_stopping_words(network: NetworkType) -> String {
    format!(
        "Your node on {} has not stopped yet. The wallet's node is not started.",
        network.as_str()
    )
}

impl DaemonManager {
    /// Stop the node of this account the wallet found running, and start
    /// the wallet's own (Rick, 2026-11-17). The stop is `daemon_stop` over
    /// the owner's leg. The wallet holds no connection to that node whose
    /// end would say it is gone, so it looks at the rendezvous until it is,
    /// and then waits out that node's hold on the data directory
    /// ([`Self::launch_waiting_locked`]). Both waits end on a person's
    /// "Stop waiting" ([`Self::stop_waiting`]), which leaves the node idle.
    /// The wallet never kills a node it did not start.
    pub async fn replace_found(self: &Arc<Self>) -> Result<(), String> {
        let _lifecycle = self.lifecycle.lock().await;
        let network = {
            let inner = self.inner.read().await;
            if inner.state != NodeState::Found {
                return Err("No other node of yours is running to stop.".to_owned());
            }
            inner.network
        };
        let target = self.target(network);
        let stop = tokio::task::spawn_blocking(move || target.call::<DaemonStop>(&NoParams))
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = stop {
            let words = format!(
                "Your node on {} did not take the stop: {e}",
                network.as_str()
            );
            if let shekyl_rpc_transport::JsonRpcError::Transport(fault) = &e {
                if fault.is_security_failure() {
                    self.set_state(NodeState::Refused(fault.to_string())).await;
                }
            }
            return Err(words);
        }
        self.stop_waiting.send_replace(false);
        self.set_state(NodeState::Stopping).await;
        self.inner.write().await.waits_on_another = true;
        tracing::info!(network = network.as_str(), "stopping the found node");
        let replaced = self.replace_stopping_locked(network).await;
        self.inner.write().await.waits_on_another = false;
        replaced
    }

    /// The found node took its stop: wait for it to be gone from "this
    /// computer", then start the wallet's own.
    async fn replace_stopping_locked(self: &Arc<Self>, network: NetworkType) -> Result<(), String> {
        loop {
            match self.dial(network).await {
                Ok(()) | Err(LocalFault::Unreachable(_)) => {
                    if self.asked_to_stop_waiting(FOUND_NODE_POLL).await {
                        self.leave_waiting(still_stopping_words(network)).await;
                        return Ok(());
                    }
                }
                Err(fault) if fault.is_security_failure() => {
                    self.set_state(NodeState::Refused(fault.to_string())).await;
                    return Err(fault.to_string());
                }
                Err(_) => break,
            }
        }
        self.launch_waiting_locked().await
    }

    /// Start the wallet's own node, waiting out a hold on its data
    /// directory: while the node refuses with `config.data-dir.locked` the
    /// state is `waiting`, naming the directory, and the start is retried
    /// after 500 ms, then twice as long each time, up to a few seconds.
    /// Returns when the node is ready, when it fails for another reason,
    /// or when a person ends the wait: then nothing is started and nothing
    /// is killed, the state is `idle`, and [`Self::start_again`] is the
    /// retry.
    pub(super) async fn launch_waiting_locked(self: &Arc<Self>) -> Result<(), String> {
        let mut wait = HELD_DIR_FIRST_WAIT;
        let mut quiet = false;
        let mut held: Option<String> = None;
        loop {
            // A "Stop waiting" that came before this launch: nothing more
            // is started.
            if *self.stop_waiting.borrow() {
                self.leave_waiting(left_waiting_words(held.as_deref().unwrap_or_default()))
                    .await;
                return Ok(());
            }
            let node = self.launch_locked(quiet).await;
            let outcome = tokio::select! {
                outcome = self.ready_or_words(node) => Some(outcome),
                () = self.stop_asked() => None,
            };
            let Some(outcome) = outcome else {
                // "Stop waiting" while this launch was in flight: the node it
                // started is the wallet's own, so it is asked to stop (Force
                // stop is offered meanwhile) and the wait ends there. Nothing
                // is killed.
                self.stop_own_locked().await;
                self.leave_waiting(left_waiting_words(held.as_deref().unwrap_or_default()))
                    .await;
                return Ok(());
            };
            match outcome {
                Ok(()) => return Ok(()),
                Err(words) if words.contains(DATA_DIR_LOCKED) => {
                    held = Some(words.clone());
                    // The refused node's process is gone (its words came
                    // with the end of its connection); its follower may not
                    // have run yet, and this flow does not wait for it.
                    self.inner.write().await.node = None;
                    self.set_state(NodeState::Waiting(waiting_words(&words)))
                        .await;
                    quiet = true;
                    if self.asked_to_stop_waiting(wait).await {
                        self.leave_waiting(left_waiting_words(&words)).await;
                        return Ok(());
                    }
                    wait = next_held_dir_wait(wait);
                }
                Err(words) => {
                    // A quiet start's follower leaves `waiting` alone.
                    let mut inner = self.inner.write().await;
                    if matches!(inner.state, NodeState::Waiting(_)) {
                        inner.state = NodeState::Failed(words.clone());
                    }
                    return Err(words);
                }
            }
        }
    }

    /// Returns once a person asks to stop waiting.
    async fn stop_asked(&self) {
        let mut asked = self.stop_waiting.subscribe();
        drop(asked.wait_for(|asked| *asked).await);
    }

    /// Sleep `wait`, or less when a person asks to stop waiting. `true`
    /// when they asked.
    async fn asked_to_stop_waiting(&self, wait: Duration) -> bool {
        let mut asked = self.stop_waiting.subscribe();
        tokio::select! {
            () = tokio::time::sleep(wait) => false,
            stopped = asked.wait_for(|asked| *asked) => stopped.is_ok(),
        }
    }

    /// A person ended the wait: the node is idle, and `words` say why.
    async fn leave_waiting(&self, words: String) {
        tracing::info!("the wait was ended on request: {words}");
        self.set_state(NodeState::Idle).await;
        self.inner.write().await.left_waiting = Some(words);
    }

    /// End the wallet's wait on a node it did not start. Nothing is killed
    /// and nothing more is started; the flow that waited leaves the node
    /// idle. It takes no lifecycle lock: the waiting flow holds it.
    pub async fn stop_waiting(&self) -> Result<(), String> {
        if !self.inner.read().await.waits_on_another {
            return Err("The wallet is not waiting for a node.".to_owned());
        }
        self.stop_waiting.send_replace(true);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wait for a held data directory doubles from 500 ms and stops
    /// growing at a few seconds.
    #[test]
    fn the_wait_for_a_held_directory_doubles_to_a_few_seconds() {
        let mut wait = HELD_DIR_FIRST_WAIT;
        let mut waits = vec![wait];
        for _ in 0..5 {
            wait = next_held_dir_wait(wait);
            waits.push(wait);
        }
        let millis: Vec<u128> = waits.iter().map(Duration::as_millis).collect();
        assert_eq!(millis, [500, 1000, 2000, 4000, 4000, 4000]);
    }

    /// The notice names the directory out of the daemon's own sentence, as
    /// core renders `config.data-dir.locked`; a sentence it cannot read
    /// still says what is held.
    #[test]
    fn the_waiting_notice_names_the_held_directory() {
        let refusal = "shekyld: another daemon is using the data directory /home/a b/.shekyl/testnet; \
                       stop it, or give this one a different `--data-dir` [config.data-dir.locked from machine]";
        assert_eq!(held_directory(refusal), Some("/home/a b/.shekyl/testnet"));
        let words = waiting_words(refusal);
        assert!(
            words.contains("/home/a b/.shekyl/testnet")
                && words.contains("The node you stopped still holds the data directory"),
            "{words}"
        );
        assert!(left_waiting_words(refusal).contains("/home/a b/.shekyl/testnet"));
        assert_eq!(held_directory("[config.data-dir.locked]"), None);
        assert!(waiting_words("[config.data-dir.locked]").contains("still holds"));
    }
}
