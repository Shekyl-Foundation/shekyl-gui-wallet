// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! A Tor fix taken, kept and put back, against a running `shekyld`
//! (`docs/design/GUI_SYNC_LANE.md` GS-3). `#[ignore]`, and run with
//! [`crate::live_daemon_tests`].
//!
//! No node here reaches a network: clearnet is off throughout, the build
//! under test has no Tor beside its node, and the one operator's Tor named
//! is a closed port on this computer.

use std::ffi::OsString;
use std::net::TcpListener;
use std::path::Path;

use shekyl_rpc_client::wire::DaemonStatus;
use shekyl_rpc_types::kinds::NoParams;
use shekyl_tor_status::{Fix, TorSetting, TorStatus};

use crate::daemon_manager::{DaemonConfig, DaemonManager};
use crate::live_daemon_tests::{free_port, run_dir};
use crate::live_own_node_tests::{manager_with, process_goes, state_becomes};
use crate::state::NetworkType;
use crate::tor_fix::TorFix;

/// The arguments of a node whose Tor is the launch line's to say: clearnet
/// off, and nothing about Tor.
fn clearnet_off(dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["--clearnet", "off", "--log-file"]
        .map(OsString::from)
        .into();
    args.push(dir.join("shekyld.log").into());
    args
}

/// The Tor status of the wallet's node, read on its owner's leg.
async fn tor(dm: &DaemonManager) -> TorStatus {
    let leg = dm.own_target().await.expect("the wallet's node is ready");
    tokio::task::spawn_blocking(move || leg.call::<DaemonStatus>(&NoParams))
        .await
        .unwrap()
        .expect("daemon_status")
        .tor_status()
        .expect("the owner's leg is shown the Status part")
}

/// A port on this computer nothing listens on.
fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// **A fix is taken from the read, kept in the wallet's settings, passed
/// on the launch line, and put back.** A node with no Tor beside it runs a
/// managed Tor that is missing: its read offers the fixes, `operator`
/// among them. The person takes `operator`: the node is a new process
/// whose Tor setting is `operator`, its read offers no fix, and the
/// setting is in `daemon.json`, so a wallet started later launches the
/// same. A value the node refuses leaves the node down with its own words
/// and the setting kept; from there the way back still works, and the node
/// runs its managed Tor again. Red edit: in `tor_args`, drop the
/// `Operator` arm's arguments (the node stays `managed`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_tor_fix_is_taken_kept_and_put_back() {
    let dir = run_dir();
    let port = free_port();
    let (dm, _gate) = manager_with(&dir, port, clearnet_off(&dir));
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;

    let before = tor(&dm).await;
    assert_eq!(before.posture.tor, TorSetting::Managed);
    let offered = before.tor.as_ref().expect("a managed Tor").fix.clone();
    assert!(
        offered.contains(&Fix::Operator) && offered.contains(&Fix::ClearnetOnly),
        "a node with no Tor beside it offers the fixes: {before:?}"
    );
    assert!(!before.own_tx_hides_address());
    let first = dm.node_pid().await.expect("the node's pid");

    let tor_socks = format!("127.0.0.1:{}", closed_port());
    let fix = TorFix::Operator {
        tor_socks: tor_socks.clone(),
    };
    dm.set_tor_fix(Some(fix.clone()))
        .await
        .expect("the node starts again with the fix");
    let second = dm.node_pid().await.expect("the new node's pid");
    assert_ne!(first, second, "a new process");
    let after = tor(&dm).await;
    assert_eq!(after.posture.tor, TorSetting::Operator, "{after:?}");
    assert!(after.tor.is_none(), "no managed Tor, so no fix is offered");
    assert!(!after.own_tx_hides_address(), "nothing answers at that Tor");
    let settings = dir.join("settings");
    assert_eq!(DaemonConfig::load(&settings).tor_fix, Some(fix.clone()));

    // A wallet started later launches with the fix it kept.
    dm.shutdown().await;
    process_goes(second, &dir).await;
    let (dm, _gate) = manager_with(&dir, port, clearnet_off(&dir));
    assert_eq!(dm.config().await.tor_fix, Some(fix));
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    assert_eq!(tor(&dm).await.posture.tor, TorSetting::Operator);

    // A value the node refuses: its words, the node down, the setting kept.
    let refused = TorFix::Operator {
        tor_socks: "not an address".to_owned(),
    };
    let words = dm
        .set_tor_fix(Some(refused.clone()))
        .await
        .expect_err("the node refuses that address");
    assert!(words.contains("tor-socks"), "the node's own words: {words}");
    assert_eq!(dm.status().await.state, "failed");
    assert_eq!(DaemonConfig::load(&settings).tor_fix, Some(refused));

    // The way back works with the node down.
    dm.set_tor_fix(None).await.expect("the bundled Tor again");
    assert_eq!(dm.status().await.state, "running");
    let back = tor(&dm).await;
    assert_eq!(back.posture.tor, TorSetting::Managed);
    assert_eq!(DaemonConfig::load(&settings).tor_fix, None);

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}
