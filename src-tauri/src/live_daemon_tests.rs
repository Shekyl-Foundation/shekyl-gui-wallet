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
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use shekyl_rpc_client::wire::block::{BlockSelect, HeaderSelect};
use shekyl_rpc_client::wire::chain::CurveSelect;
use shekyl_rpc_client::wire::shared::Selector;
use shekyl_rpc_client::wire::{
    BlockHeaderShow, CurveShow, DaemonStatus, MineStatus, ShardCoverage, Version,
};
use shekyl_rpc_types::kinds::NoParams;

use crate::daemon_manager::{sidecar_args, DaemonConfig};
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
        keep_running_on_exit: false,
        data_dir: Some(dir.join("data").to_string_lossy().into_owned()),
    };
    let args = sidecar_args(&config, network, port, None);
    launch(args, dir, port).await
}

async fn launch(mut args: Vec<OsString>, dir: PathBuf, port: u16) -> Daemon {
    args.extend(["--clearnet", "off", "--tor", "off", "--log-file"].map(OsString::from));
    args.push(dir.join("shekyld.log").into());
    let child = Command::new(sidecar())
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the bundled shekyld starts");
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

/// The wallet's own node does not let it control mining: the view
/// listener holds no `mining-control`, and the wallet says so in words.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn the_wallet_s_own_node_refuses_mining_control_in_words() {
    let d = start().await;
    let refused = daemon_rpc::call::<MineStatus>(&d.url, NoParams, "mining")
        .await
        .expect_err("mine_status is outside the view listener's grants");
    assert_eq!(
        refused,
        "This node does not let this wallet see or change mining."
    );
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
