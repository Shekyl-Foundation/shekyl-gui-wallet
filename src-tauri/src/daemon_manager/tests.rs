// Copyright (c) 2026, The Shekyl Foundation
// BSD-3-Clause license (see LICENSE)

use super::*;
use shekyl_engine_core::Network;

fn config(data_dir: Option<&str>) -> DaemonConfig {
    DaemonConfig {
        data_dir: data_dir.map(str::to_owned),
        ..DaemonConfig::default()
    }
}

fn names(tor_file: Option<PathBuf>) -> LaunchNames {
    LaunchNames {
        tor_file,
        ..LaunchNames::default()
    }
}

#[test]
fn the_sidecar_names_the_bundled_tor() {
    let tor = bundled_tor(Path::new("/res"));
    let args = sidecar_args(
        &config(None),
        NetworkType::Testnet,
        12029,
        &names(Some(tor.clone())),
    );
    let want: Vec<OsString> = [
        "--network",
        "testnet",
        "--rpc-admin-listen",
        "none",
        "--rpc-view-listen",
        "127.0.0.1:12029",
        "--clearnet-listen",
        "none",
        "--tor-file",
    ]
    .iter()
    .map(OsString::from)
    .chain([tor.clone().into_os_string()])
    .collect();
    assert_eq!(args, want);
    assert_eq!(tor, Path::new("/res").join("tor").join(TOR_FILE));
}

/// Every network's launch names that network and listens on its port
/// from core's row. Red edit: drop `--network` (the node then runs
/// mainnet whatever the wallet's network).
#[test]
fn the_launch_carries_the_wallet_s_network_and_its_port() {
    for net in NetworkType::ALL {
        let args = sidecar_args(&config(None), net, net.rpc_port(), &names(None));
        assert_eq!(
            args[..2],
            [OsString::from("--network"), OsString::from(net.as_str())]
        );
        assert_eq!(
            args[5],
            OsString::from(format!("127.0.0.1:{}", net.network().ports().rpc))
        );
    }
    assert_eq!(
        NetworkType::default(),
        NetworkType::Mainnet,
        "Rick, 2026-11-13"
    );
}

/// A configured data directory is per network: the root for mainnet,
/// the network's subdirectory from core's row otherwise, so a switch
/// never points two networks at one store.
#[test]
fn a_configured_data_dir_is_per_network() {
    let root = Path::new("/data");
    let dirs: Vec<PathBuf> = NetworkType::ALL
        .iter()
        .map(|n| data_dir_for(root, *n))
        .collect();
    for (n, dir) in NetworkType::ALL.iter().zip(&dirs) {
        let want = n
            .network()
            .subdirectory()
            .map_or_else(|| root.to_path_buf(), |sub| root.join(sub));
        assert_eq!(*dir, want);
    }
    let distinct: std::collections::BTreeSet<_> = dirs.iter().collect();
    assert_eq!(
        distinct.len(),
        dirs.len(),
        "one store per network: {dirs:?}"
    );
    let args = sidecar_args(
        &config(Some("/data")),
        NetworkType::Testnet,
        12029,
        &names(None),
    );
    let at = args.iter().position(|a| a == "--data-dir").unwrap();
    assert_eq!(
        PathBuf::from(&args[at + 1]),
        data_dir_for(root, NetworkType::Testnet)
    );
}

/// The node follows a switch only while no wallet of another network is
/// open: a wallet of one network never gets a node of another.
#[test]
fn a_wallet_of_one_network_never_gets_a_node_of_another() {
    assert!(may_switch(None, NetworkType::Testnet).is_ok());
    assert!(may_switch(Some(Network::Testnet), NetworkType::Testnet).is_ok());
    let refused = may_switch(Some(Network::Testnet), NetworkType::Mainnet).unwrap_err();
    assert!(
        refused.contains("Close it before switching to mainnet"),
        "{refused}"
    );
}

#[test]
fn without_a_resource_directory_no_tor_file_is_named() {
    let args = sidecar_args(&config(None), NetworkType::Mainnet, 11029, &names(None));
    assert!(!args.iter().any(|a| a == "--tor-file"));
}

/// The miner's address rides the launch line: the daemon reads it only
/// at its start. And the wallet's launch never opens an admin listener:
/// its own calls go over the owner's leg.
#[test]
fn the_launch_names_the_mining_address_and_no_admin_listener() {
    let args = sidecar_args(
        &config(None),
        NetworkType::Testnet,
        12029,
        &LaunchNames {
            mine_address: Some("SKL1abc".to_owned()),
            ..LaunchNames::default()
        },
    );
    let at = args.iter().position(|a| a == "--mine-address").unwrap();
    assert_eq!(args[at + 1], OsString::from("SKL1abc"));
    let admin = args.iter().position(|a| a == "--rpc-admin-listen").unwrap();
    assert_eq!(args[admin + 1], OsString::from("none"));
    // No inbound clearnet listener (Rick, 2026-11-17).
    let listen = args.iter().position(|a| a == "--clearnet-listen").unwrap();
    assert_eq!(args[listen + 1], OsString::from("none"));
    // Turned on in Settings, the node keeps the daemon's own listener on
    // the network's peer port: the launch names none. Red edit: push
    // `--clearnet-listen none` whatever the setting.
    let on = DaemonConfig {
        clearnet_inbound: true,
        ..DaemonConfig::default()
    };
    let args = sidecar_args(&on, NetworkType::Testnet, 12029, &LaunchNames::default());
    assert!(!args.iter().any(|a| a == "--clearnet-listen"), "{args:?}");
    // A settings file from before the setting reads as off.
    let old: DaemonConfig = serde_json::from_str(r#"{"data_dir": null}"#).unwrap();
    assert!(!old.clearnet_inbound);
    assert!(
        !args.iter().any(|a| a == "--owning-connection"),
        "the launcher's"
    );
}

/// An old settings file that still carries the keep-running key loads:
/// the key is not a setting any more, and is not an error either.
#[test]
fn a_settings_file_with_the_retired_key_still_loads() {
    let config: DaemonConfig =
        serde_json::from_str(r#"{"keep_running_on_exit": true, "data_dir": "/d"}"#).unwrap();
    assert_eq!(config.data_dir.as_deref(), Some("/d"));
}

/// A test's own directory, removed when it goes.
#[cfg(unix)]
struct TestDir(PathBuf);

#[cfg(unix)]
impl TestDir {
    fn new(what: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "shekyl-gui-{what}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("s")).unwrap();
        std::fs::set_permissions(dir.join("s"), std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(dir)
    }
}

#[cfg(unix)]
impl Drop for TestDir {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// The engine's way to the node on this computer is open only while the
/// node there is the wallet's own. Before the start it is closed; with
/// nothing at "this computer" and no node in the build, the wallet
/// starts nothing, says so, and the way stays closed, with the same
/// words. Red edit: close the gate only for `Found` and `Refused`, as it
/// first was; the engine could then dial whatever listens at the port.
/// Unix: the test names its own socket directory, which Windows refuses.
#[cfg(unix)]
#[tokio::test]
async fn the_engine_s_way_is_closed_unless_the_node_is_the_wallet_s_own() {
    let dir = TestDir::new("gate");
    let gate = NodeGate::default();
    let env = LaunchEnv {
        program: None,
        tor_file: None,
        socket_dir: Some(dir.0.join("s")),
        instance: None,
        rpc_port: None,
        extra_args: Vec::new(),
    };
    let state = crate::state::AppState {
        node_gate: gate.clone(),
        ..crate::state::AppState::new()
    };
    assert!(state.engine_base().await.is_ok(), "no manager yet");
    let dm = DaemonManager::new(dir.0.join("settings"), env, gate);
    assert_eq!(
        state.engine_base().await.err().as_deref(),
        Some(NODE_NOT_STARTED),
        "closed before the start"
    );
    assert_eq!(dm.status().await.state, "idle");

    dm.start(NetworkType::Testnet).await;
    let status = dm.status().await;
    assert_eq!(status.state, "unavailable", "{status:?}");
    let words = status.message.expect("why");
    assert_eq!(state.engine_base().await.err(), Some(words.clone()));
    assert_eq!(dm.own_target().await.err(), Some(words));
    assert!(!status.can_force_stop);

    // A node address a person typed is theirs: the gate is about the
    // node on this computer alone.
    *state.typed_node.write().await = true;
    assert!(state.engine_base().await.is_ok());
}

/// The engine's way to this computer waits while the node starts, and
/// opens on the ready node's own address, never on the configured one. A
/// closed gate says why. Red edit: answer `Gate::Starting` with `Ok` at
/// once (the engine would dial a port the node may not hold yet).
#[tokio::test]
async fn the_engine_waits_while_the_node_starts_and_gets_the_ready_node_s_address() {
    use crate::state::Gate;
    let gate = NodeGate::default();
    assert_eq!(gate.passable().await, Ok(None), "no manager: as configured");

    gate.set(Gate::Starting);
    let waiting = {
        let gate = gate.clone();
        tokio::spawn(async move { gate.passable().await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !waiting.is_finished(),
        "a caller waits while the node starts"
    );
    gate.set(Gate::Open("http://127.0.0.1:12029".to_owned()));
    assert_eq!(
        waiting.await.unwrap(),
        Ok(Some("http://127.0.0.1:12029".to_owned()))
    );

    gate.set(Gate::Starting);
    let waiting = {
        let gate = gate.clone();
        tokio::spawn(async move { gate.passable().await })
    };
    gate.set(Gate::Closed("The wallet's node stopped.".to_owned()));
    assert_eq!(
        waiting.await.unwrap(),
        Err("The wallet's node stopped.".to_owned())
    );
}

/// A program that starts and never says it is ready: it ignores its
/// arguments and its owning connection, as a hung node would.
#[cfg(unix)]
fn hung_node(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("hung-node");
    std::fs::write(&path, "#!/bin/sh\nexec sleep 600\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// **"Stop waiting" ends the wait while a retried launch is in flight**
/// (finding on #71). The wallet waits out a held data directory and its
/// launch never says it is ready. Stop waiting asks the node it just
/// started to stop and shows `stopping` with Force stop, since that node is
/// the wallet's own; it does not stop on the ask, so the person forces it,
/// and the wait ends idle with nothing left running. Red edit: in
/// `launch_waiting_locked`, wait on `ready_or_words` alone (the wait never
/// ends while the launch hangs).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn stop_waiting_ends_a_wait_whose_launch_never_says_ready() {
    let dir = TestDir::new("hung");
    let env = LaunchEnv {
        program: Some(hung_node(&dir.0)),
        tor_file: None,
        socket_dir: Some(dir.0.join("s")),
        instance: None,
        rpc_port: Some(1),
        extra_args: Vec::new(),
    };
    let dm = DaemonManager::new(dir.0.join("settings"), env, NodeGate::default());
    // As after a person ended an earlier wait: Retry waits out the held
    // directory again.
    dm.inner.write().await.left_waiting = Some("held".to_owned());
    let retry = {
        let dm = Arc::clone(&dm);
        tokio::spawn(async move { dm.start_again().await })
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while dm.node_pid().await.is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the launch never came"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pid = dm.node_pid().await.expect("the hung node");
    assert!(dm.status().await.can_stop_waiting);

    dm.stop_waiting().await.expect("the wait is ended");
    while dm.status().await.state != "stopping" {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            dm.status().await
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(dm.status().await.can_force_stop, "the wallet's own node");
    dm.force_stop().await.expect("the person forces it");
    tokio::time::timeout(std::time::Duration::from_secs(20), retry)
        .await
        .expect("the wait ends once the node is gone")
        .unwrap();
    let status = dm.status().await;
    assert_eq!(status.state, "idle", "{status:?}");
    assert_eq!(dm.node_pid().await, None);
    assert!(
        std::fs::metadata(format!("/proc/{pid}")).is_err() || !cfg!(target_os = "linux"),
        "nothing left running"
    );
}
