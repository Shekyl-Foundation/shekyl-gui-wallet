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

use crate::daemon_rpc;

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

/// Start the sidecar as the wallet does (`daemon_manager::sidecar_args`'s
/// listeners: no admin listener, one view listener on loopback), on
/// regtest with no network, and wait for its `version` read.
async fn start() -> Daemon {
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free loopback port")
        .port();
    let dir = std::env::temp_dir().join(format!(
        "shekyl-gui-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let child = Command::new(sidecar())
        .args(["--network", "regtest", "--clearnet", "off", "--tor", "off"])
        .args(["--rpc-admin-listen", "none"])
        .arg("--rpc-view-listen")
        .arg(format!("127.0.0.1:{port}"))
        .arg("--data-dir")
        .arg(dir.join("data"))
        .arg("--log-file")
        .arg(dir.join("shekyld.log"))
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
