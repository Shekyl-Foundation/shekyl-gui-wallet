// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The wallet's own node, launched as the wallet launches it (GS-2),
//! against a running `shekyld`: the sidecar this build bundles.
//!
//! `#[ignore]`, and run with [`crate::live_daemon_tests`]: see that
//! module for where the sidecar comes from.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use shekyl_rpc_client::wire::{DaemonStatus, MineStatus, Version};
use shekyl_rpc_types::kinds::NoParams;

use crate::daemon_manager::{
    data_dir_for, sidecar_args, test_instance, DaemonConfig, DaemonManager, LaunchEnv, LaunchNames,
};
use crate::daemon_rpc;
use crate::live_daemon_tests::{free_port, run_dir, sidecar, socket_dir};
use crate::state::NetworkType;

/// This process's role when it is a child of one of the tests below.
const ROLE: &str = "SHEKYL_GUI_LIVE_ROLE";
/// The run directory the roles share.
const ROLE_DIR: &str = "SHEKYL_GUI_LIVE_DIR";
/// The view-listener port the roles share.
const ROLE_PORT: &str = "SHEKYL_GUI_LIVE_PORT";

/// How long a node may take to say it is ready, or to be gone after it is
/// asked. A test's bound on a wait the wallet itself does not bound.
const SETTLES: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(100);

/// The arguments that keep a test's node off every network.
///
/// With clearnet off, core refuses any `clearnet-listen` at all
/// (`config.value.conflict`), `none` included, so the harness's node
/// settings say `clearnet_inbound: true`: the launch then names no
/// listener, and with clearnet off nothing listens. The wallet's own
/// default (`--clearnet-listen none`) is held by the launch-line unit tests.
fn no_traffic(dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["--clearnet", "off", "--tor", "off", "--log-file"]
        .map(OsString::from)
        .into();
    args.push(dir.join("shekyld.log").into());
    args
}

/// The wallet's daemon manager over the bundled node, in `dir`: its own
/// socket directory (Unix; Windows refuses one, so there the tests share
/// the account's rendezvous and run one at a time), its own data directory
/// and its own port.
pub(crate) fn manager(
    dir: &Path,
    port: u16,
) -> (std::sync::Arc<DaemonManager>, crate::state::NodeGate) {
    manager_with(dir, port, no_traffic(dir), None)
}

/// [`manager`], with the test's own further arguments for the node.
pub(crate) fn manager_with(
    dir: &Path,
    port: u16,
    extra_args: Vec<OsString>,
    tor_file: Option<PathBuf>,
) -> (std::sync::Arc<DaemonManager>, crate::state::NodeGate) {
    let config_dir = dir.join("settings");
    // A second manager in the same directory reads what the first kept.
    if !config_dir.join("daemon.json").exists() {
        DaemonConfig {
            data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
            clearnet_inbound: true,
            ..DaemonConfig::default()
        }
        .save(&config_dir)
        .unwrap();
    }
    let env = LaunchEnv {
        program: Some(sidecar()),
        tor_file,
        socket_dir: cfg!(unix).then(|| socket_dir(dir)),
        instance: cfg!(windows).then(|| test_instance(port)),
        rpc_port: Some(port),
        extra_args,
    };
    let gate = crate::state::NodeGate::default();
    (DaemonManager::new(config_dir, env, gate.clone()), gate)
}

/// Wait until the manager's state is `want`, and panic with the state and
/// the node's log when it does not come.
pub(crate) async fn state_becomes(dm: &DaemonManager, dir: &Path, want: &str) {
    let deadline = Instant::now() + SETTLES;
    loop {
        let status = dm.status().await;
        if status.state == want {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the node's state is {status:?}, not {want}; its log ends:\n{}",
            log_tail(dir)
        );
        tokio::time::sleep(POLL).await;
    }
}

fn log_tail(dir: &Path) -> String {
    let log = std::fs::read_to_string(dir.join("shekyld.log")).unwrap_or_default();
    let tail: Vec<&str> = log.lines().rev().take(20).collect();
    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
}

/// Whether a process with this id exists.
#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    // Signal 0: the existence check alone. A zombie still exists, and a
    // node's zombie would be this test's own unreaped child; the wallet
    // reaps its node, and a node of another process is init's to reap.
    let pid = i32::try_from(pid).expect("a pid");
    // SAFETY: `kill` with signal 0 sends nothing.
    #[allow(unsafe_code, reason = "kill(pid, 0) is the existence check")]
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    let mut command = Command::new("tasklist");
    command.args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"]);
    let out = shekyl_spawn::spawn_lock::with(|| command.output()).expect("tasklist");
    String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
}

/// Wait until no process has this id.
pub(crate) async fn process_goes(pid: u32, dir: &Path) {
    let deadline = Instant::now() + SETTLES;
    while process_exists(pid) {
        assert!(
            Instant::now() < deadline,
            "the node (pid {pid}) is still there; its log ends:\n{}",
            log_tail(dir)
        );
        tokio::time::sleep(POLL).await;
    }
}

/// **The wallet's node, on its owner's leg.** The node the wallet launches
/// says it is ready, and the wallet's own calls are admitted on the owner's
/// leg: `mine_status` (`mining-control`, which the view listener refuses),
/// and `daemon_status` with its Status part shown, the Tor fields among
/// them. Then the miner's address: the node is stopped and started again
/// with it, the new start waiting only for the old node to be gone, and
/// `mine_status` names it. Red edit: launch without `--owning-connection`
/// (no ready line: the state never leaves `starting`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn the_wallet_s_node_answers_on_its_owner_s_leg_and_relaunches_for_mining() {
    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    assert!(dm.status().await.can_force_stop);

    let target = dm.own_target().await.expect("the wallet's node is ready");
    let leg = target.clone();
    let mining = tokio::task::spawn_blocking(move || leg.call::<MineStatus>(&NoParams))
        .await
        .unwrap()
        .expect("mine_status is admitted on the owner's leg");
    assert_eq!(mining.address.0, None, "no wallet's address yet");
    let leg = target.clone();
    let status = tokio::task::spawn_blocking(move || leg.call::<DaemonStatus>(&NoParams))
        .await
        .unwrap()
        .expect("daemon_status");
    assert!(
        status.tor_status().is_some(),
        "the owner's leg is shown the Status part"
    );

    let address = wallet_address(&dir, "live", port).await;

    let first = dm.node_pid().await.expect("the node's pid");
    dm.mine_to(&address).await.expect("the node starts again");
    let second = dm.node_pid().await.expect("the new node's pid");
    assert_ne!(first, second, "a new process");
    assert!(
        !process_exists(first),
        "the first node is gone before the second starts"
    );
    let leg = dm.own_target().await.expect("ready again");
    let mining = tokio::task::spawn_blocking(move || leg.call::<MineStatus>(&NoParams))
        .await
        .unwrap()
        .expect("mine_status");
    assert_eq!(mining.address.0.map(|a| a.0), Some(address.clone()));
    // The same address again is no relaunch.
    dm.mine_to(&address).await.expect("already paying it");
    assert_eq!(dm.node_pid().await, Some(second));

    dm.shutdown().await;
    process_goes(second, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// The address of a new wallet of the node's network, created against the
/// wallet's node and closed: an address the node accepts as its miner's.
async fn wallet_address(dir: &Path, name: &str, port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/json_rpc");
    let password = format!("live-{}-{port}", std::process::id());
    let mut session = crate::engine_session::EngineSession::new();
    session
        .create(
            &dir.join("wallets"),
            name,
            &password,
            NetworkType::Testnet,
            &url,
        )
        .await
        .expect("a wallet created against the wallet's node");
    let address = session.primary_address().await.expect("its address");
    session.close().await.expect("the wallet closes");
    address
}

/// The miner of the wallet's node, read on its owner's leg.
async fn miner(dm: &DaemonManager) -> shekyl_rpc_client::wire::node::MineStatus {
    let leg = dm.own_target().await.expect("the wallet's node is ready");
    tokio::task::spawn_blocking(move || leg.call::<MineStatus>(&NoParams))
        .await
        .unwrap()
        .expect("mine_status")
}

/// **A wallet opened while the node is not mining causes no restart**
/// (Rick, 2026-11-17). The node pays one wallet's address and its miner is
/// stopped. Another wallet is opened: the node is the same process, still
/// paying the first address. `mine_to`, which Start mining calls, is what
/// moves it. Red edit: in `follow_wallet`, drop the `!miner.active` return
/// (the open restarts the node, then fails asking a miner that had no
/// threads to start again: `mine.threads-zero`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_wallet_opened_while_not_mining_causes_no_restart() {
    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dir, "first", port).await;
    let second = wallet_address(&dir, "second", port).await;
    assert_ne!(first, second);

    // A node that pays no address yet is left alone too.
    let pid = dm.node_pid().await.expect("the node's pid");
    dm.follow_wallet(&first).await.expect("nothing to do");
    assert_eq!(dm.node_pid().await, Some(pid));

    dm.mine_to(&first).await.expect("the node pays the first");
    let paying = dm.node_pid().await.expect("the node's pid");
    assert!(!miner(&dm).await.active, "the miner was never started");

    dm.follow_wallet(&second)
        .await
        .expect("a wallet opened while not mining");
    assert_eq!(dm.node_pid().await, Some(paying), "no restart");
    assert_eq!(dm.status().await.state, "running");
    assert_eq!(miner(&dm).await.address.0.map(|a| a.0), Some(first));

    // Start mining is what moves it.
    dm.mine_to(&second).await.expect("the node pays the second");
    assert_ne!(dm.node_pid().await, Some(paying));
    assert_eq!(miner(&dm).await.address.0.map(|a| a.0), Some(second));

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A wallet opened while the miner runs moves the miner to it.** The
/// node's miner is running and pays one wallet's address. Another wallet is
/// opened: the node is started again paying that wallet, and its miner is
/// running again with the threads it had. Red edit: in `follow_wallet`,
/// drop the `mine_start` after the relaunch (the miner is left stopped).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_wallet_opened_while_the_miner_runs_moves_the_miner_to_it() {
    use shekyl_rpc_client::wire::node::{MineStart, Threads};
    use shekyl_rpc_types::kinds::Count;

    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dir, "first", port).await;
    let second = wallet_address(&dir, "second", port).await;

    dm.mine_to(&first).await.expect("the node pays the first");
    let leg = dm.own_target().await.expect("ready");
    tokio::task::spawn_blocking(move || {
        leg.call::<shekyl_rpc_client::wire::MineStart>(&MineStart {
            threads: Threads::Count(Count(1)),
        })
    })
    .await
    .unwrap()
    .expect("mine_start on the owner's leg");
    let before = miner(&dm).await;
    assert!(before.active, "the miner runs: {before:?}");
    let paying = dm.node_pid().await.expect("the node's pid");

    dm.follow_wallet(&second)
        .await
        .expect("a wallet opened while mining");
    assert_ne!(dm.node_pid().await, Some(paying), "a new process");
    assert!(!process_exists(paying));
    let after = miner(&dm).await;
    assert!(after.active, "the miner goes on: {after:?}");
    assert_eq!(after.threads.0, before.threads.0);
    assert_eq!(after.address.0.map(|a| a.0), Some(second));

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A held data directory is waited out, and the wait can be ended**
/// (Rick, 2026-11-17). A node of this account runs at the wallet's "this
/// computer" in a data directory of its own, and this test process holds
/// the lock of the wallet's data directory as a daemon does. The found node
/// is stopped on request and goes; the wallet's own start is refused for
/// the held directory, and the notice names the directory and says the
/// stopped node still holds it. "Stop waiting" ends the wait: the state is
/// idle, nothing was killed (the lock is still held, and the found node's
/// exit was its own clean stop), and nothing of the wallet's runs. Retry
/// waits again, and starts the wallet's node once the lock is released.
/// Red edit: in `launch_waiting_locked`, sleep in place of
/// `asked_to_stop_waiting` (the replacement does not return when the wait
/// is ended).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_held_data_directory_is_waited_out_and_the_wait_can_be_ended() {
    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager(&dir, port);
    let held = data_dir_for(&dir.join("data"), NetworkType::Testnet);
    let lock = shekyl_daemon_config::Lock::take(&held).expect("the test holds the lock");

    // The found node: the wallet's rendezvous and port, its own directory.
    let names = LaunchNames {
        socket_dir: cfg!(unix).then(|| socket_dir(&dir)),
        instance: cfg!(windows).then(|| test_instance(port)),
        ..LaunchNames::default()
    };
    let config = DaemonConfig {
        data_dir: Some(dir.join("hand").to_string_lossy().into_owned()),
        clearnet_inbound: true,
        ..DaemonConfig::default()
    };
    let mut args = sidecar_args(&config, NetworkType::Testnet, port, &names);
    args.extend(no_traffic(&dir));
    let mut command = Command::new(sidecar());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut hand = shekyl_spawn::spawn_lock::with(|| command.spawn()).expect("the node starts");
    let url = format!("http://127.0.0.1:{port}/json_rpc");
    let deadline = Instant::now() + SETTLES;
    while daemon_rpc::call::<Version>(&url, NoParams, "the node's version")
        .await
        .is_err()
    {
        assert!(
            Instant::now() < deadline,
            "the hand-started node: {}",
            log_tail(&dir)
        );
        tokio::time::sleep(POLL).await;
    }
    dm.start(NetworkType::Testnet).await;
    assert_eq!(dm.status().await.state, "found");
    assert!(!dm.status().await.can_stop_waiting);
    assert!(
        dm.stop_waiting().await.is_err(),
        "nothing is waited for yet"
    );

    let replacing = {
        let dm = std::sync::Arc::clone(&dm);
        tokio::spawn(async move { dm.replace_found().await })
    };
    state_becomes(&dm, &dir, "waiting").await;
    let status = dm.status().await;
    let words = status.message.clone().expect("the wait is said");
    let shown = held.display().to_string();
    assert!(
        words.contains(&shown) && words.contains("The node you stopped still holds"),
        "{words}"
    );
    assert!(status.can_stop_waiting, "{status:?}");
    let exit = hand.wait().expect("the found node's exit");
    assert!(exit.success(), "the found node's own clean stop: {exit}");

    // The wait holds for as long as the lock does: starts are refused at
    // about 0.5 s and 1.5 s, and this lands inside the 2 s wait that
    // follows.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(dm.status().await.state, "waiting");

    dm.stop_waiting().await.expect("the wait is ended");
    tokio::time::timeout(Duration::from_secs(30), replacing)
        .await
        .expect("the replacement returns when the wait is ended")
        .unwrap()
        .expect("ending the wait is no failure");
    let status = dm.status().await;
    assert_eq!(status.state, "idle", "{status:?}");
    assert!(!status.can_stop_waiting && !status.can_force_stop);
    let words = status.message.expect("why it is idle");
    assert!(words.contains(&shown), "{words}");
    assert_eq!(dm.node_pid().await, None, "nothing of the wallet's runs");
    assert!(
        shekyl_daemon_config::Lock::take(&held).is_err(),
        "the holder was left alone: the directory is still held"
    );

    // Retry waits again while the directory is held, and starts the
    // wallet's node once it is released.
    let retrying = {
        let dm = std::sync::Arc::clone(&dm);
        tokio::spawn(async move { dm.start_again().await })
    };
    state_becomes(&dm, &dir, "waiting").await;
    drop(lock);
    retrying.await.unwrap();
    assert_eq!(dm.status().await.state, "running");
    assert_eq!(dm.status().await.message, None);

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **With testnet saved, the first node the app launches is a testnet
/// node, and no mainnet node runs at any point** (Rick, 2026-11-17). The
/// app reads the saved network from `gui-config.json` before anything
/// starts (`run`: `gui_config::load().startup_network()`), its state is on
/// that network, and the node is started on it. This drives the same
/// steps: the file is saved with testnet, read back, and the manager
/// started on what it gives. Red edit: `GuiConfig::startup_network`
/// returns the default (the first launch is mainnet).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn with_testnet_saved_the_first_node_is_a_testnet_node() {
    let dir = run_dir();
    let port = free_port();
    let file = dir.join("gui-config.json");
    crate::gui_config::save_to(
        &crate::gui_config::GuiConfig {
            network: Some(NetworkType::Testnet.as_str().to_owned()),
            ..crate::gui_config::GuiConfig::default()
        },
        &file,
    );
    let network = crate::gui_config::load_from(&file).startup_network();
    let state = crate::state::AppState::on(network);
    assert_eq!(*state.network.read().await, NetworkType::Testnet);

    let (dm, _gate) = manager(&dir, port);
    dm.start(network).await;
    state_becomes(&dm, &dir, "running").await;
    assert_eq!(
        dm.launched(),
        [NetworkType::Testnet],
        "one launch, on testnet: no mainnet node ran first"
    );
    assert_eq!(dm.status().await.network, "testnet");
    // The node answers at testnet's rendezvous, and nothing is at
    // mainnet's: each network's record has its own name.
    let leg = dm.own_target().await.expect("the wallet's node");
    tokio::task::spawn_blocking(move || leg.call::<DaemonStatus>(&NoParams))
        .await
        .unwrap()
        .expect("the node answers at testnet's rendezvous");
    let instance = cfg!(windows).then(|| crate::daemon_manager::test_instance(port));
    let mainnet = shekyl_rpc_transport::local::LocalTarget::resolve(
        NetworkType::Mainnet.network(),
        &crate::daemon_manager::test_instance_of(instance.as_deref()),
        cfg!(unix).then(|| socket_dir(&dir)).as_deref(),
    )
    .expect("mainnet's rendezvous");
    assert!(
        matches!(
            tokio::task::spawn_blocking(move || mainnet.dial().map(drop))
                .await
                .unwrap(),
            Err(shekyl_rpc_transport::local::LocalFault::NotRunning { .. })
        ),
        "no mainnet node"
    );

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A hand-started node is found, stopped on request, and replaced**
/// (Rick, 2026-11-17). A node of this account already runs on the network,
/// started with no owning connection, in the data directory the wallet's
/// own node uses. The wallet finds it and does not adopt it: no call of the
/// wallet's goes to it, and the engine's URL is closed. On request it is
/// stopped over the owner's leg and the wallet's own node starts, waiting
/// out the stopped node's hold on the data directory. Red edit: in
/// `start_locked`, launch without dialing first (the wallet's start meets
/// the found node's lock and fails, never `found`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_hand_started_node_is_found_stopped_on_request_and_replaced() {
    let dir = run_dir();
    let port = free_port();
    let (dm, gate) = manager(&dir, port);

    // The hand-started node: the wallet's own command line, by hand.
    let names = LaunchNames {
        socket_dir: cfg!(unix).then(|| socket_dir(&dir)),
        instance: cfg!(windows).then(|| test_instance(port)),
        ..LaunchNames::default()
    };
    let config = DaemonConfig {
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
        clearnet_inbound: true,
        ..DaemonConfig::default()
    };
    let mut args = sidecar_args(&config, NetworkType::Testnet, port, &names);
    args.extend(no_traffic(&dir));
    let mut command = Command::new(sidecar());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut hand = shekyl_spawn::spawn_lock::with(|| command.spawn()).expect("the node starts");
    let url = format!("http://127.0.0.1:{port}/json_rpc");
    let deadline = Instant::now() + SETTLES;
    while daemon_rpc::call::<Version>(&url, NoParams, "the node's version")
        .await
        .is_err()
    {
        assert!(
            Instant::now() < deadline,
            "the hand-started node: {}",
            log_tail(&dir)
        );
        tokio::time::sleep(POLL).await;
    }

    dm.start(NetworkType::Testnet).await;
    let status = dm.status().await;
    assert_eq!(status.state, "found", "{status:?}");
    assert!(!status.can_force_stop, "not the wallet's to kill");
    let words = status.message.expect("the found node is said");
    assert!(words.contains("already running on testnet"), "{words}");
    assert_eq!(
        dm.own_target().await.err(),
        Some(words.clone()),
        "not adopted"
    );
    let state = crate::state::AppState::new();
    *state.network.write().await = NetworkType::Testnet;
    *state.daemon_url.write().await = NetworkType::Testnet.local_url();
    // The app's state carries the manager's gate.
    let state = crate::state::AppState {
        node_gate: gate,
        ..state
    };
    assert_eq!(
        state.engine_base().await.err(),
        Some(words),
        "the engine's URL is closed"
    );

    dm.replace_found().await.expect("stopped and replaced");
    assert_eq!(dm.status().await.state, "running");
    let exit = hand.wait().expect("the hand-started node's exit");
    assert!(exit.success(), "a clean stop: {exit}");
    assert!(
        state.engine_base().await.is_ok(),
        "the engine's URL is open again"
    );
    let leg = dm.own_target().await.expect("the wallet's own node");
    tokio::task::spawn_blocking(move || leg.call::<MineStatus>(&NoParams))
        .await
        .unwrap()
        .expect("the wallet's own node answers on its owner's leg");

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A start that meets a held data directory fails in the node's own
/// words, and those words carry the code the replacement waits on.** A node
/// started by hand holds the wallet's data directory from another socket
/// directory, so nothing answers at the wallet's "this computer" and the
/// wallet launches. The launch is refused, the state is `failed` with the
/// daemon's sentence, and the sentence carries `config.data-dir.locked`:
/// the code `replace_found` reads to start again while a stopped node still
/// holds the directory. The engine's way stays closed. Unix: Windows has
/// one rendezvous an account, so a second node there is found, not missed.
/// Red edit: change `DATA_DIR_LOCKED` by a letter.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_held_data_directory_refuses_the_start_in_the_node_s_words() {
    let dir = run_dir();
    let port = free_port();
    let (dm, gate) = manager(&dir, port);

    // The same data directory, another socket directory and port.
    let other = dir.join("other");
    let names = LaunchNames {
        socket_dir: Some(socket_dir(&other)),
        ..LaunchNames::default()
    };
    let config = DaemonConfig {
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
        clearnet_inbound: true,
        ..DaemonConfig::default()
    };
    let hand_port = free_port();
    let mut args = sidecar_args(&config, NetworkType::Testnet, hand_port, &names);
    args.extend(no_traffic(&other));
    let mut command = Command::new(sidecar());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut hand = shekyl_spawn::spawn_lock::with(|| command.spawn()).expect("the node starts");
    let url = format!("http://127.0.0.1:{hand_port}/json_rpc");
    let deadline = Instant::now() + SETTLES;
    while daemon_rpc::call::<Version>(&url, NoParams, "the node's version")
        .await
        .is_err()
    {
        assert!(
            Instant::now() < deadline,
            "the hand-started node: {}",
            log_tail(&other)
        );
        tokio::time::sleep(POLL).await;
    }

    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "failed").await;
    let words = dm.status().await.message.expect("the node's words");
    assert!(
        words.contains(crate::daemon_manager::DATA_DIR_LOCKED),
        "the refusal carries its code: {words}"
    );
    assert!(
        words.contains("another daemon is using the data directory"),
        "{words}"
    );
    let state = crate::state::AppState {
        node_gate: gate,
        ..crate::state::AppState::new()
    };
    assert_eq!(state.engine_base().await.err(), Some(words));

    drop(hand.kill());
    drop(hand.wait());
    drop(std::fs::remove_dir_all(&dir));
}

/// The wallet as a process of its own, for the exit tests: start the node,
/// say its pid, then exit cleanly (`exit`) or wait to be killed (`hang`).
async fn wallet_role(role: &str) -> ! {
    let dir = PathBuf::from(std::env::var_os(ROLE_DIR).expect("the run directory"));
    let port: u16 = std::env::var(ROLE_PORT).unwrap().parse().unwrap();
    let (dm, _gate) = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    println!("node {}", dm.node_pid().await.expect("the node's pid"));
    match role {
        // What the app does when its window is destroyed, and no more.
        "exit" => {
            dm.shutdown().await;
            std::process::exit(0)
        }
        _ => loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        },
    }
}

/// Start `test` again as the wallet in `role`, and read the node's pid.
fn spawn_wallet(test: &str, role: &str, dir: &Path, port: u16) -> (Child, u32) {
    use std::io::BufRead;
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args(["--exact", test, "--nocapture", "--include-ignored"])
        .env(ROLE, role)
        .env(ROLE_DIR, dir)
        .env(ROLE_PORT, port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut wallet = shekyl_spawn::spawn_lock::with(|| command.spawn()).expect("the wallet role");
    let lines = std::io::BufReader::new(wallet.stdout.take().unwrap()).lines();
    let pid = lines
        .map_while(Result::ok)
        .find_map(|l| l.strip_prefix("node ").and_then(|p| p.trim().parse().ok()))
        .unwrap_or_else(|| {
            drop(wallet.kill());
            panic!(
                "the wallet role never named its node; the node's log ends:\n{}",
                log_tail(dir)
            )
        });
    (wallet, pid)
}

/// After the wallet is gone, its node is too: the process, its rendezvous
/// and its hold on the data directory. A second wallet in the same
/// directories starts its own node, which a node left behind would refuse.
async fn assert_no_node_is_left(node: u32, dir: &Path, port: u16) {
    process_goes(node, dir).await;
    let (dm, _gate) = manager(dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, dir, "running").await;
    let pid = dm.node_pid().await.expect("the second wallet's node");
    dm.shutdown().await;
    process_goes(pid, dir).await;
}

/// **A clean wallet exit leaves no node behind.** The wallet asks its node
/// to stop and exits without waiting; nothing kills the node, and it stops
/// on its own: a clean stop, announced in its log. Red edit: launch with
/// the node's end kept open in a second process (the node outlives the
/// wallet).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_clean_wallet_exit_leaves_no_node_behind() {
    const NAME: &str = "live_own_node_tests::a_clean_wallet_exit_leaves_no_node_behind";
    if let Ok(role) = std::env::var(ROLE) {
        wallet_role(&role).await;
    }
    let dir = run_dir();
    let port = free_port();
    let (mut wallet, node) = spawn_wallet(NAME, "exit", &dir, port);
    assert!(wallet.wait().expect("the wallet's exit").success());
    assert_no_node_is_left(node, &dir, port).await;
    let log = std::fs::read_to_string(dir.join("shekyld.log")).unwrap_or_default();
    assert!(
        log.contains("the process that started this daemon closed its owning connection"),
        "the node stopped on the close, cleanly; its log ends:\n{}",
        log_tail(&dir)
    );
    drop(std::fs::remove_dir_all(&dir));
}

/// **A killed wallet cannot orphan its node.** The wallet is killed with no
/// chance to ask anything; its death closes the owning connection, and the
/// node stops on its own. Red edit: launch the node without
/// `--owning-connection` (it outlives the killed wallet, holding the data
/// directory).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_killed_wallet_cannot_orphan_its_node() {
    const NAME: &str = "live_own_node_tests::a_killed_wallet_cannot_orphan_its_node";
    if let Ok(role) = std::env::var(ROLE) {
        wallet_role(&role).await;
    }
    let dir = run_dir();
    let port = free_port();
    let (mut wallet, node) = spawn_wallet(NAME, "hang", &dir, port);
    wallet.kill().expect("the wallet is killed");
    drop(wallet.wait());
    assert_no_node_is_left(node, &dir, port).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **Force stop is the kill, and the node starts again after it.** A
/// person's force stop kills the wallet's node; the state says the node
/// stopped, and a start brings up a new one.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_force_stop_kills_the_node_and_it_starts_again() {
    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = dm.node_pid().await.expect("the node's pid");
    dm.force_stop().await.expect("the kill");
    state_becomes(&dm, &dir, "failed").await;
    assert!(!process_exists(first));
    assert!(!dm.status().await.can_force_stop);
    dm.start_again().await;
    state_becomes(&dm, &dir, "running").await;
    let second = dm.node_pid().await.expect("the new node's pid");
    dm.shutdown().await;
    process_goes(second, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}
