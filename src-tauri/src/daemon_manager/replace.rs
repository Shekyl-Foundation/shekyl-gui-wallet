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

use super::{standing, DaemonManager, NodeState, Standing, DATA_DIR_LOCKED};
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
            if let shekyl_rpc_transport::JsonRpcError::Transport(fault @ LocalFault::PeerCheck(_)) =
                &e
            {
                self.refuse(fault.to_string()).await;
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
    /// computer", then start the wallet's own. Only a dial that says no
    /// node is there starts one ([`standing`]): a service that came up
    /// meanwhile, a failed check, or another fault is the state, in its
    /// words, and nothing is started beside it.
    async fn replace_stopping_locked(self: &Arc<Self>, network: NetworkType) -> Result<(), String> {
        loop {
            match self.dial(network).await {
                // Still there, or still closing its endpoint.
                Ok(()) | Err(LocalFault::Unreachable(_)) => {
                    if self.asked_to_stop_waiting(FOUND_NODE_POLL).await {
                        self.leave_waiting(still_stopping_words(network)).await;
                        return Ok(());
                    }
                }
                Err(fault) => match standing(fault) {
                    Standing::Nothing => break,
                    Standing::Blocked(state) => return Err(self.block(network, state).await),
                },
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
            let outcome = unless_stopped(self.stop_asked(), self.ready_or_words(node)).await;
            let words = match outcome {
                // A ready node ends the wait, unless a "Stop waiting" was
                // accepted first ([`Self::wait_is_over`]).
                Some(Ok(())) if self.wait_is_over().await => return Ok(()),
                Some(Ok(())) | None => {
                    // "Stop waiting" while this launch was in flight: the
                    // node it started is the wallet's own, so it is asked to
                    // stop (Force stop is offered meanwhile) and the wait
                    // ends there. Nothing is killed.
                    self.stop_own_locked().await;
                    self.leave_waiting(left_waiting_words(held.as_deref().unwrap_or_default()))
                        .await;
                    return Ok(());
                }
                Some(Err(words)) => words,
            };
            if !words.contains(DATA_DIR_LOCKED) {
                // A quiet start's follower leaves `waiting` alone.
                let mut inner = self.inner.write().await;
                if matches!(inner.state, NodeState::Waiting(_)) {
                    inner.state = NodeState::Failed(words.clone());
                }
                return Err(words);
            }
            // The refused node's process is gone (its words came with the
            // end of its connection); its follower may not have run yet,
            // and this flow does not wait for it.
            self.inner.write().await.node = None;
            self.set_state(NodeState::Waiting(waiting_words(&words)))
                .await;
            quiet = true;
            if self.asked_to_stop_waiting(wait).await {
                self.leave_waiting(left_waiting_words(&words)).await;
                return Ok(());
            }
            held = Some(words);
            wait = next_held_dir_wait(wait);
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

    /// The launch this flow waited on is ready: the wait is over, unless a
    /// person's "Stop waiting" was accepted first. One critical section
    /// with [`Self::stop_waiting`], so a request it accepted is never
    /// lost to a node that became ready at the same moment: either the
    /// request came first and this returns `false` (the flow then stops
    /// the node it started), or the wait ended first and the request is
    /// refused.
    async fn wait_is_over(&self) -> bool {
        let mut inner = self.inner.write().await;
        if *self.stop_waiting.borrow() {
            return false;
        }
        inner.waits_on_another = false;
        true
    }

    /// End the wallet's wait on a node it did not start. Nothing is killed
    /// and nothing more is started; the flow that waited leaves the node
    /// idle. It takes no lifecycle lock: the waiting flow holds it.
    pub async fn stop_waiting(&self) -> Result<(), String> {
        // Held across the send: see [`Self::wait_is_over`].
        let inner = self.inner.read().await;
        if !inner.waits_on_another {
            return Err("The wallet is not waiting for a node.".to_owned());
        }
        self.stop_waiting.send_replace(true);
        Ok(())
    }
}

/// `work`'s result, or `None` when `stop` completes: and when both are
/// complete at once, `None`. The stop is polled first, so a request to
/// stop is never passed over for work that finished in the same moment.
async fn unless_stopped<T>(
    stop: impl std::future::Future<Output = ()>,
    work: impl std::future::Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        biased;
        () = stop => None,
        done = work => Some(done),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon_manager::LaunchEnv;

    /// **A request to stop is never passed over for work that finished in
    /// the same moment.** Both complete at once, every time: the stop is
    /// taken. Red edit: drop `biased;` from `unless_stopped` (the select
    /// then picks either at random, and a ready node outlives an accepted
    /// "Stop waiting").
    #[tokio::test]
    async fn a_stop_is_never_passed_over_for_work_finished_in_the_same_moment() {
        for _ in 0..256 {
            assert_eq!(unless_stopped(async {}, async { 1 }).await, None);
        }
        assert_eq!(
            unless_stopped(std::future::pending(), async { 1 }).await,
            Some(1)
        );
    }

    /// **An accepted "Stop waiting" is never lost to a node that became
    /// ready.** Accepted first, the wait is not over and the flow stops the
    /// node it started; the wait over first, the request is refused, so the
    /// page never shows a stop that was taken and then ignored. Red edit:
    /// in `wait_is_over`, drop the check of the request.
    #[tokio::test]
    async fn an_accepted_stop_waiting_is_never_lost_to_a_ready_node() {
        let dm = DaemonManager::new(
            std::env::temp_dir().join("shekyl-gui-none"),
            LaunchEnv::default(),
        );
        assert!(dm.stop_waiting().await.is_err(), "nothing waits");

        dm.inner.write().await.waits_on_another = true;
        dm.stop_waiting().await.expect("accepted while waiting");
        assert!(!dm.wait_is_over().await, "the accepted request stands");

        dm.stop_waiting.send_replace(false);
        assert!(dm.wait_is_over().await, "no request: the wait is over");
        assert!(
            dm.stop_waiting().await.is_err(),
            "a request after the wait is over is refused, not lost"
        );
    }

    /// **A service that appears while the found node stops is not launched
    /// beside.** The found node is gone and a service's record stands at
    /// "this computer": the replacement starts no node, the state is
    /// `service`, and the words are core's. Red edit: in
    /// `replace_stopping_locked`, break to the launch on any fault that is
    /// not a failed check. Unix: the test names its own socket directory.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_service_that_appears_while_the_found_node_stops_is_not_launched_beside() {
        use shekyl_rpc_rendezvous::{service_record_path, Instance, ServiceRecord};
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "shekyl-gui-replace-service-{}-{}",
            std::process::id(),
            crate::engine_session::test_password()
        ));
        let services = dir.join("services");
        std::fs::create_dir_all(&services).unwrap();
        std::fs::create_dir_all(dir.join("s")).unwrap();
        std::fs::set_permissions(dir.join("s"), std::fs::Permissions::from_mode(0o700)).unwrap();
        let view: std::net::SocketAddr = "127.0.0.1:12030".parse().unwrap();
        let network = NetworkType::Testnet;
        std::fs::write(
            service_record_path(
                &services,
                &network.network().network_id(),
                &Instance::Default,
            ),
            ServiceRecord {
                channel: None,
                view: Some(view),
            }
            .bytes(),
        )
        .unwrap();
        let env = LaunchEnv {
            // A program that is never run: the test fails if anything launches.
            program: Some(dir.join("no-node-here")),
            tor_file: None,
            service_rendezvous: Some(services),
            socket_dir: Some(dir.join("s")),
            instance: None,
            extra_args: Vec::new(),
        };
        let dm = DaemonManager::new(dir.join("settings"), env);
        dm.inner.write().await.network = network;

        let said = dm
            .replace_stopping_locked(network)
            .await
            .expect_err("no node is started beside a service");
        assert_eq!(said, shekyl_rpc_client::service_refusal(Some(view)));
        let status = dm.status().await;
        assert_eq!(status.state, "service", "{status:?}");
        assert!(dm.launched().is_empty(), "nothing was launched");
        drop(std::fs::remove_dir_all(&dir));
    }

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
