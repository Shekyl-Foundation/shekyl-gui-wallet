// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The wallet's daemon reads against a running `shekyld`: the sidecar this
//! build bundles, started the way the wallet starts it.
//!
//! `#[ignore]`: the fast CI job has no daemon. The release job places
//! core's gitian `shekyld` in `binaries/` and runs these with `--ignored`,
//! so every release dry run checks the wallet against the daemon it ships
//! with, not against vectors alone. Run locally by placing a `shekyld`
//! built from the paired core at `binaries/shekyld-<target triple>`.

use std::ffi::OsString;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use shekyl_rpc_client::wire::block::{BlockSelect, HeaderSelect};
use shekyl_rpc_client::wire::chain::CurveSelect;
use shekyl_rpc_client::wire::shared::Selector;
use shekyl_rpc_client::wire::{
    BlockHeaderShow, CurveShow, DaemonStatus, MineStatus, ShardCoverage, Version,
};
use shekyl_rpc_types::kinds::NoParams;

use crate::daemon_manager::{sidecar_args, DaemonConfig, DaemonManager, LaunchEnv, LaunchNames};
use crate::daemon_rpc;
use crate::state::NetworkType;

/// The bundled sidecar: the one non-empty `binaries/shekyld-*` (CI's
/// placeholder is empty).
fn sidecar() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("shekyld-"))
                && std::fs::metadata(p).is_ok_and(|m| m.len() > 0)
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "exactly one bundled shekyld in {}: {found:?}",
        dir.display()
    );
    found.into_iter().next().unwrap()
}

/// A running regtest daemon, stopped and its directory removed on drop.
struct Daemon {
    child: Child,
    dir: PathBuf,
    url: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A fresh directory for one run's node.
fn run_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "shekyl-gui-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A free loopback port.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free loopback port")
        .port()
}

/// Regtest with no network, on the wallet's listeners (no admin listener,
/// one view listener on loopback). The wallet has no regtest network, so
/// this is the harness's own command line for the read tests.
async fn start() -> Daemon {
    let port = free_port();
    let dir = run_dir();
    let mut args: Vec<OsString> = ["--network", "regtest", "--rpc-admin-listen", "none"]
        .map(OsString::from)
        .into();
    args.push("--rpc-view-listen".into());
    args.push(format!("127.0.0.1:{port}").into());
    args.push("--data-dir".into());
    args.push(dir.join("data").into());
    launch(args, dir, port).await
}

/// The node the wallet launches on `network`: `daemon_manager::sidecar_args`
/// itself, at a free port, with a configured data directory, and no traffic
/// (no clearnet, no Tor), so the chain is its genesis block alone.
async fn start_on(network: NetworkType) -> Daemon {
    let port = free_port();
    let dir = run_dir();
    let config = DaemonConfig {
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
    };
    let args = sidecar_args(&config, network, port, &LaunchNames::default());
    launch(args, dir, port).await
}

async fn launch(mut args: Vec<OsString>, dir: PathBuf, port: u16) -> Daemon {
    args.extend(["--clearnet", "off", "--tor", "off", "--log-file"].map(OsString::from));
    args.push(dir.join("shekyld.log").into());
    let mut command = Command::new(sidecar());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Under the process's one spawn lock, as every spawn is in a process
    // that hands a node its owning connection.
    let child =
        shekyl_spawn::spawn_lock::with(|| command.spawn()).expect("the bundled shekyld starts");
    let daemon = Daemon {
        child,
        dir,
        url: format!("http://127.0.0.1:{port}/json_rpc"),
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if daemon_rpc::call::<Version>(&daemon.url, NoParams, "the node's version")
            .await
            .is_ok()
        {
            return daemon;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    // The directory goes with the guard, so the log's tail rides in the
    // message.
    let log = std::fs::read_to_string(daemon.dir.join("shekyld.log")).unwrap_or_default();
    let tail: Vec<&str> = log.lines().rev().take(20).collect();
    panic!(
        "the bundled shekyld did not answer its version read within 60 s; its log ends:\n{}",
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    );
}

/// The node the wallet launches runs the wallet's network, on every
/// network: its own `version` read names it. Red edit: drop `--network`
/// from `sidecar_args` (every node then runs mainnet).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn the_wallet_s_node_runs_the_wallet_s_network() {
    for network in NetworkType::ALL {
        let d = start_on(network).await;
        let version = daemon_rpc::call::<Version>(&d.url, NoParams, "the node's version")
            .await
            .expect("version");
        assert_eq!(version.network, network.network(), "{}", network.as_str());
    }
}

/// Every read the wallet makes decodes from the bundled daemon, through
/// the listener the wallet uses.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn the_wallet_s_reads_decode_from_the_bundled_daemon() {
    let d = start().await;
    let status = daemon_rpc::call::<DaemonStatus>(&d.url, NoParams, "the node's status")
        .await
        .expect("daemon_status");
    assert!(status.offline, "started with no network");
    let tip = HeaderSelect(BlockSelect {
        select: Selector::Tip,
        pow: false,
        prune: None,
    });
    let header = daemon_rpc::call::<BlockHeaderShow>(&d.url, tip, "the last block")
        .await
        .expect("block_header_show at the tip");
    assert_eq!(header.height, status.height, "the tip the status names");
    daemon_rpc::call::<CurveShow>(&d.url, CurveSelect(Selector::Tip), "the curve tree")
        .await
        .expect("curve_show at the tip");
    daemon_rpc::call::<ShardCoverage>(&d.url, NoParams, "the shard coverage")
        .await
        .expect("shard_coverage");
}

/// A node address a person typed is called on its view listener, which
/// holds no `mining-control`: the wallet says so in words. (The wallet's own
/// node is called on its owner's leg, below.)
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_view_listener_refuses_mining_control_in_words() {
    let d = start().await;
    let refused = daemon_rpc::call::<MineStatus>(&d.url, NoParams, "mining")
        .await
        .expect_err("mine_status is outside the view listener's grants");
    assert_eq!(
        refused,
        "This node does not let this wallet see or change mining."
    );
}

// ─── The wallet's own node, launched as the wallet launches it (GS-2) ─────

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

/// The run's directories: the node's socket directory (Unix), owner-only
/// as the daemon requires, and the wallet's settings directory.
fn socket_dir(dir: &Path) -> PathBuf {
    let s = dir.join("s");
    std::fs::create_dir_all(&s).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    s
}

/// The arguments that keep a test's node off every network.
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
fn manager(dir: &Path, port: u16) -> (std::sync::Arc<DaemonManager>, crate::state::NodeGate) {
    let config_dir = dir.join("settings");
    DaemonConfig {
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
    }
    .save(&config_dir)
    .unwrap();
    let env = LaunchEnv {
        program: Some(sidecar()),
        tor_file: None,
        socket_dir: cfg!(unix).then(|| socket_dir(dir)),
        rpc_port: Some(port),
        extra_args: no_traffic(dir),
    };
    let gate = crate::state::NodeGate::default();
    (DaemonManager::new(config_dir, env, gate.clone()), gate)
}

/// Wait until the manager's state is `want`, and panic with the state and
/// the node's log when it does not come.
async fn state_becomes(dm: &DaemonManager, dir: &Path, want: &str) {
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
async fn process_goes(pid: u32, dir: &Path) {
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
/// `mine_status` names it. Red edits: launch without `--owning-connection`
/// (no ready line: the state never leaves `starting`); or run against a
/// daemon that closes its owning end when its stop begins (the second start
/// meets a locked data directory and fails).
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

    // A wallet of the node's network, for an address the node accepts.
    let url = format!("http://127.0.0.1:{port}/json_rpc");
    let password = format!("live-{}-{port}", std::process::id());
    let mut session = crate::engine_session::EngineSession::new();
    session
        .create(
            &dir.join("wallets"),
            "live",
            &password,
            NetworkType::Testnet,
            &url,
        )
        .await
        .expect("a wallet created against the wallet's node");
    let address = session.primary_address().await.expect("its address");
    session.close().await.expect("the wallet closes");

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
        ..LaunchNames::default()
    };
    let config = DaemonConfig {
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
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
    const NAME: &str = "live_daemon_tests::a_clean_wallet_exit_leaves_no_node_behind";
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
    const NAME: &str = "live_daemon_tests::a_killed_wallet_cannot_orphan_its_node";
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

/// The wallet's own pathway against the bundled node: a wallet created
/// through `EngineSession` refreshes, and on a chain that is its genesis
/// block alone it has scanned nothing (a wallet starts from genesis and scans
/// from block 1, `local_refresh.rs`), so its count is 0 and the page says
/// "nothing scanned yet" beside the node's block 0.
///
/// What this cannot show is a synced wallet naming its node's block past
/// genesis: that needs blocks, the levers that make them serve only regtest
/// (NI-R6), and the wallet has no regtest network. That equality is held by
/// `blockNumberOfCount`'s tests (1200 blocks end at block 1199), the chain
/// read's vector test (tip 1199 shows 1199) and core's
/// `synced_count_is_the_daemon_count_of_the_scanned_chain`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_wallet_on_a_genesis_only_chain_has_scanned_nothing() {
    let d = start_on(NetworkType::Testnet).await;
    let wallets = d.dir.join("wallets");
    // A throwaway wallet in the run's own directory: its password is made
    // per run, never written into the source.
    let password = format!(
        "live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut session = crate::engine_session::EngineSession::new();
    session
        .create(&wallets, "live", &password, NetworkType::Testnet, &d.url)
        .await
        .expect("a wallet created against the bundled node");
    let shared = session.shared_engine().expect("the wallet is open");
    // The create's own catch-up keeps going past a failed refresh (it logs
    // and leaves the wallet open); this one must succeed, so a failure is
    // the test's, in its own words.
    let summary = shekyl_engine_core::Engine::start_refresh(
        shared.clone(),
        shekyl_engine_core::RefreshOptions::default(),
    )
    .await
    .expect("a refresh starts")
    .join()
    .await
    .expect("the wallet refreshes against the bundled node");
    let synced = shared
        .read()
        .await
        .ledger()
        .ledger
        .synced_count()
        .expect("a chain count")
        .to_raw();
    let status = daemon_rpc::call::<DaemonStatus>(&d.url, NoParams, "the node's status")
        .await
        .expect("daemon_status");
    assert_eq!(status.height.0, 0, "no traffic: the chain is genesis alone");
    assert_eq!(summary.blocks_processed, 0, "nothing past genesis to scan");
    assert_eq!(synced, 0, "the wallet's count: nothing scanned");
    // A close waits for every holder of the engine: this test's is the last.
    drop(shared);
    session.close().await.expect("the wallet closes");
}
