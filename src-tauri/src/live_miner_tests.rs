// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! What the wallet's own node's miner pays, against a running `shekyld`
//! (GS-2): Start mining, a wallet opened beside a miner, a network switch,
//! and a node the wallet cannot read.
//!
//! `#[ignore]`, and run with [`crate::live_daemon_tests`]: see that
//! module for where the sidecar comes from. The node is launched as
//! [`crate::live_own_node_tests`] launches it.

use std::time::{Duration, Instant};

use shekyl_rpc_client::wire::MineStatus;
use shekyl_rpc_types::kinds::NoParams;

use crate::daemon_manager::DaemonManager;
use crate::live_daemon_tests::{free_port, run_dir};
use crate::live_own_node_tests::{
    manager, process_exists, process_goes, state_becomes, wallet_address, POLL, SETTLES,
};
use crate::state::NetworkType;

/// The miner of the wallet's node, read on its owner's leg.
async fn miner(dm: &DaemonManager) -> shekyl_rpc_client::wire::node::MineStatus {
    let leg = dm.own_target().await.expect("the wallet's node is ready");
    tokio::task::spawn_blocking(move || {
        leg.call::<MineStatus>(&NoParams, shekyl_rpc_transport::local::CALL_WITHIN)
    })
    .await
    .unwrap()
    .expect("mine_status")
}

/// **A wallet opened while the node is not mining causes no restart**
/// (Rick, 2026-11-17). The node pays one wallet's address and its miner is
/// stopped. Another wallet is opened: the node is the same process, still
/// paying the first address. Start mining is what moves it (`mine_to` is
/// its restart half). Red edit: in `reconcile_payout`, drop the `running_miner` check
/// (the open restarts the node, then fails asking a miner that had no
/// threads to start again: `mine.threads-zero`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_wallet_opened_while_not_mining_causes_no_restart() {
    let dir = run_dir();
    let port = free_port();
    let dm = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dm, &dir, "first").await;
    let second = wallet_address(&dm, &dir, "second").await;
    assert_ne!(first, second);

    // A node that pays no address yet is left alone too.
    let pid = dm.node_pid().await.expect("the node's pid");
    dm.wallet_is_reconciled(&first).await;
    assert_eq!(dm.node_pid().await, Some(pid));

    dm.mine_to(&first).await.expect("the node pays the first");
    let paying = dm.node_pid().await.expect("the node's pid");
    assert!(!miner(&dm).await.active, "the miner was never started");

    dm.wallet_is_reconciled(&second).await;
    assert_eq!(dm.payout_fault().await, None);
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
/// running again with the threads it had. Red edit: in
/// `reconcile_payout`, drop the `start_miner` after the relaunch (the miner
/// is left stopped).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_wallet_opened_while_the_miner_runs_moves_the_miner_to_it() {
    use shekyl_rpc_client::wire::node::{MineStart, Threads};
    use shekyl_rpc_types::kinds::Count;

    let dir = run_dir();
    let port = free_port();
    let dm = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dm, &dir, "first").await;
    let second = wallet_address(&dm, &dir, "second").await;

    dm.mine_to(&first).await.expect("the node pays the first");
    let leg = dm.own_target().await.expect("ready");
    tokio::task::spawn_blocking(move || {
        leg.call::<shekyl_rpc_client::wire::MineStart>(
            &MineStart {
                threads: Threads::Count(Count(1)),
            },
            shekyl_rpc_transport::local::CALL_WITHIN,
        )
    })
    .await
    .unwrap()
    .expect("mine_start on the owner's leg");
    let before = miner(&dm).await;
    assert!(before.active, "the miner runs: {before:?}");
    let paying = dm.node_pid().await.expect("the node's pid");

    dm.wallet_is_reconciled(&second).await;
    assert_eq!(dm.payout_fault().await, None, "the miner moved");
    assert_ne!(dm.node_pid().await, Some(paying), "a new process");
    assert!(!process_exists(paying));
    let after = miner(&dm).await;
    assert!(after.active, "the miner goes on: {after:?}");
    assert_eq!(after.threads.0, before.threads.0);
    assert_eq!(after.address.0.map(|a| a.0), Some(second.clone()));

    // A hand-off asked for by an earlier open finds the later wallet: the
    // first is opened again and, before its reconciling runs, the second.
    // Both reconcile against the wallet open when they run, and the miner
    // ends on the second (finding on #71: a delayed hand-off for an earlier
    // wallet resumed after a later one).
    dm.wallet_is(Some(first.clone())).await;
    dm.wallet_is_reconciled(&second).await;
    let settled = Instant::now() + SETTLES;
    loop {
        let now = miner(&dm).await;
        if now.address.0.as_ref().map(|a| a.0.as_str()) == Some(second.as_str()) && now.active {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let later = miner(&dm).await;
            assert_eq!(
                later.address.0.map(|a| a.0),
                Some(second.clone()),
                "it stays"
            );
            break;
        }
        assert!(Instant::now() < settled, "the miner never settled: {now:?}");
        tokio::time::sleep(POLL).await;
    }

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A network switch drops the miner's address** (finding on #71). An
/// address belongs to one network, and a node started on another refuses it
/// at its start. The node pays a testnet wallet; the switch to stagenet
/// starts the stagenet node, which runs. Red edit: in `relaunch`, keep
/// `mine_address` (the stagenet node refuses its start).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_network_switch_drops_the_miner_s_address() {
    let dir = run_dir();
    let port = free_port();
    let dm = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let address = wallet_address(&dm, &dir, "testnet").await;
    dm.mine_to(&address)
        .await
        .expect("the node pays a testnet wallet");

    dm.relaunch(NetworkType::Stagenet).await;
    state_becomes(&dm, &dir, "running").await;
    assert_eq!(dm.status().await.network, "stagenet");
    assert_eq!(miner(&dm).await.address.0, None, "no address carried over");

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **Start mining pays the wallet open when it runs, in one transaction**
/// (finding on #71). The manager reads the open wallet under the lock that
/// reconciling and settings restarts take, restarts the node with its
/// address when it pays another, and starts the miner. The first wallet is
/// open and mining starts: the miner runs and pays it. The miner is
/// stopped and the second wallet opened, its reconciling still to run:
/// Start mining pays the second, whichever of the two runs first, and it
/// stays. Red edit: in `start_mining`, take the address from before the
/// lock (pass the first wallet's in): the miner starts on a wallet no
/// longer open.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn start_mining_pays_the_wallet_open_when_it_runs() {
    use shekyl_rpc_client::wire::node::Threads;
    use shekyl_rpc_client::wire::MineStop;
    use shekyl_rpc_types::kinds::Count;

    let dir = run_dir();
    let port = free_port();
    let dm = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dm, &dir, "first").await;
    let second = wallet_address(&dm, &dir, "second").await;

    dm.wallet_is_reconciled(&first).await;
    dm.start_mining(Threads::Count(Count(1)))
        .await
        .expect("mining starts for the first wallet");
    let mining = miner(&dm).await;
    assert!(mining.active, "the miner runs: {mining:?}");
    assert_eq!(mining.address.0.map(|a| a.0), Some(first.clone()));

    let leg = dm.own_target().await.expect("ready");
    tokio::task::spawn_blocking(move || {
        leg.call::<MineStop>(&NoParams, shekyl_rpc_transport::local::CALL_WITHIN)
    })
    .await
    .unwrap()
    .expect("mine_stop on the owner's leg");

    // The second wallet opens, and its reconciling is only spawned.
    dm.wallet_is(Some(second.clone())).await;
    dm.start_mining(Threads::Count(Count(1)))
        .await
        .expect("mining starts for the wallet open now");
    let settled = Instant::now() + SETTLES;
    loop {
        let now = miner(&dm).await;
        if now.active && now.address.0.as_ref().map(|a| a.0.as_str()) == Some(second.as_str()) {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let later = miner(&dm).await;
            assert!(later.active, "it goes on: {later:?}");
            assert_eq!(later.address.0.map(|a| a.0), Some(second.clone()));
            break;
        }
        assert!(Instant::now() < settled, "the miner never settled: {now:?}");
        tokio::time::sleep(POLL).await;
    }
    assert_eq!(dm.payout_fault().await, None);

    let pid = dm.node_pid().await.expect("the node's pid");
    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}

/// **A node held behind a failed check is never read as "not mining"**
/// (finding on #71). The wallet's node runs and pays one wallet; a failed
/// peer check is latched, which leaves the node alive and unreadable.
/// Another wallet opens: the wallet cannot tell whether that node mines, so
/// it says the miner may still be paying the wallet open before, and never
/// clears the warning as if nothing mined. Red edit: in `running_miner`,
/// read `refused` with a node held as `Ok(None)`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the bundled shekyld sidecar"]
async fn a_node_held_behind_a_failed_check_is_never_read_as_not_mining() {
    let dir = run_dir();
    let port = free_port();
    let dm = manager(&dir, port);
    dm.start(NetworkType::Testnet).await;
    state_becomes(&dm, &dir, "running").await;
    let first = wallet_address(&dm, &dir, "first").await;
    let second = wallet_address(&dm, &dir, "second").await;
    dm.mine_to(&first).await.expect("the node pays the first");
    let pid = dm.node_pid().await.expect("the node's pid");

    dm.refuse("the check failed, for this test".to_owned())
        .await;
    assert_eq!(dm.status().await.state, "refused");
    assert!(dm.status().await.can_force_stop, "the node is still held");

    dm.wallet_is_reconciled(&second).await;
    let fault = dm.payout_fault().await.expect("the page is told");
    assert!(
        fault.contains("may still be paying the wallet open before")
            && fault.contains("the check failed, for this test"),
        "{fault}"
    );
    assert!(process_exists(pid), "the node was not killed");

    dm.shutdown().await;
    process_goes(pid, &dir).await;
    drop(std::fs::remove_dir_all(&dir));
}
