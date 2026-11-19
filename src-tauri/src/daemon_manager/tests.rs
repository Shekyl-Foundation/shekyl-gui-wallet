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
        &names(Some(tor.clone())),
    );
    let want: Vec<OsString> = [
        "--config-file",
        "none",
        "--network",
        "testnet",
        "--rpc-admin-listen",
        "none",
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

/// Every network's launch reads no configuration file, names that
/// network, and opens no view listener. `config-file none` keeps the node
/// to this command line and the daemon's defaults, whatever a service
/// install keeps in the shared file (Rick, 2026-11-18); the wallet and its
/// engine read the node over its owner's leg (Rick, 2026-11-18), and the
/// daemon's default is no view listener. Red edits: drop `--config-file
/// none` (a service install's `tor-file` then refuses the GUI's `tor
/// operator`); drop `--network` (the node then runs mainnet whatever the
/// wallet's network); pass `--rpc-view-listen`.
#[test]
fn the_launch_reads_no_file_and_carries_the_wallet_s_network_and_no_view_listener() {
    for net in NetworkType::ALL {
        let args = sidecar_args(&config(None), net, &names(None));
        assert_eq!(
            args[..4],
            [
                OsString::from("--config-file"),
                OsString::from("none"),
                OsString::from("--network"),
                OsString::from(net.as_str())
            ]
        );
        assert!(!args.iter().any(|a| a == "--rpc-view-listen"), "{args:?}");
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
    let args = sidecar_args(&config(Some("/data")), NetworkType::Testnet, &names(None));
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
    let args = sidecar_args(&config(None), NetworkType::Mainnet, &names(None));
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
    let args = sidecar_args(&on, NetworkType::Testnet, &LaunchNames::default());
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

/// A test's own directory, removed when it goes, with a socket directory
/// `s` that is the account's alone on Unix.
struct TestDir(PathBuf);

impl TestDir {
    fn new(what: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "shekyl-gui-{what}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("s")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("s"), std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        Self(dir)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// **A service on this network: no node is launched, the words are core's,
/// and the offer points the wallet at the service's view listener** (Rick,
/// 2026-11-18). A service's rendezvous record names a view listener, and
/// no node of this account answers at "this computer": the wallet starts
/// no node beside the service (none is launched), says so in core's
/// `service_refusal`, and "Use the service's node" makes that listener the
/// typed address. Red edit: in `start_waiting_locked`, launch on
/// `LocalFault::Service` as on `NotRunning` (a node is launched beside the
/// service). Unix: the test names its own socket directory.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_service_on_the_network_launches_no_node_and_offers_its_view_listener() {
    use shekyl_rpc_rendezvous::{service_record_path, Instance, ServiceRecord};
    let dir = TestDir::new("service");
    let services = dir.0.join("services");
    std::fs::create_dir_all(&services).unwrap();
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
        program: Some(dir.0.join("no-node-here")),
        tor_file: None,
        service_rendezvous: Some(services),
        socket_dir: Some(dir.0.join("s")),
        instance: None,
        extra_args: Vec::new(),
    };
    let dm = DaemonManager::new(Ok(dir.0.join("settings")), env);
    dm.start(network).await;

    let status = dm.status().await;
    assert_eq!(status.state, "service", "{status:?}");
    assert!(
        dm.launched().is_empty(),
        "no node is launched beside a service"
    );
    assert_eq!(
        status.message.as_deref(),
        Some(shekyl_rpc_client::service_refusal(Some(view)).as_str()),
        "core's words"
    );
    assert_eq!(status.service_view.as_deref(), Some("127.0.0.1:12030"));

    let state = crate::state::AppState::on(
        network,
        crate::gui_config::Store::none(),
        crate::account_root::AccountRoot::NoHome,
    );
    let applied = crate::daemon_connection::point_at_service(&state, &dm)
        .await
        .expect("the offer");
    assert_eq!(applied.url.as_deref(), Some("http://127.0.0.1:12030"));
    assert_eq!(
        state.node_address().await.as_deref(),
        Some("http://127.0.0.1:12030")
    );
    assert!(!state.targets_this_computer().await);
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
        service_rendezvous: None,
        extra_args: Vec::new(),
    };
    let dm = DaemonManager::new(Ok(dir.0.join("settings")), env);
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

/// **Only a dial that says no node is there lets the wallet start one.**
/// Every fault of core's, each placed: the three that say nothing is there
/// start the wallet's node; a service is the `service` state with its view
/// listener; a failed check is `refused` in its own words; a node in
/// another session and one that does not answer are `failed`, and nothing
/// is started beside any of them. Red edit: in `standing`, read
/// `LocalFault::Service` as `Standing::Nothing`.
#[test]
fn only_a_dial_that_says_no_node_is_there_starts_one() {
    let node = || "testnet".to_owned();
    for fault in [
        LocalFault::NoPlace { node: node() },
        LocalFault::NotRunning { node: node() },
        LocalFault::Stale { node: node() },
    ] {
        assert_eq!(standing(fault), Standing::Nothing);
    }
    let view: std::net::SocketAddr = "127.0.0.1:12030".parse().unwrap();
    assert_eq!(
        standing(LocalFault::Service {
            node: node(),
            view: Some(view)
        }),
        Standing::Blocked(NodeState::Service(Some(view)))
    );
    assert_eq!(
        standing(LocalFault::Service {
            node: node(),
            view: None
        }),
        Standing::Blocked(NodeState::Service(None))
    );
    let check = LocalFault::PeerCheck("uid 0".to_owned());
    let words = check.to_string();
    assert_eq!(
        standing(check),
        Standing::Blocked(NodeState::Refused(words))
    );
    for fault in [
        LocalFault::OtherSession,
        LocalFault::Unreachable("timed out".to_owned()),
    ] {
        let words = fault.to_string();
        assert_eq!(standing(fault), Standing::Blocked(NodeState::Failed(words)));
    }
}

/// **A failed peer check is one latch, for the wallet's calls and its
/// engine's.** Latched in the manager, the notice says `refused` in the
/// check's words, and the engine's way to "this computer" is refused in
/// the same words with no dial: a client is not built, and the target core
/// dials itself is not handed out. A node address a person typed is theirs
/// and is not behind the latch. Red edit: in `engine_transport`, drop the
/// read of the latch (the engine dials the address the check failed at).
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_peer_check_is_one_latch_for_the_wallet_and_its_engine() {
    use crate::engine_daemon::{core_target, make_daemon, EngineNode};
    let dm = DaemonManager::new(
        Ok(std::env::temp_dir().join("shekyl-gui-none")),
        LaunchEnv::default(),
    );
    let state = crate::state::AppState::on(
        NetworkType::Testnet,
        crate::gui_config::Store::none(),
        crate::account_root::AccountRoot::NoHome,
    );
    assert_eq!(dm.refusal().await, None);

    let words = "something other than your Shekyl node is answering".to_owned();
    dm.refuse(words.clone()).await;
    let status = dm.status().await;
    assert_eq!(status.state, "refused");
    assert_eq!(status.message.as_deref(), Some(words.as_str()));

    let node = EngineNode::of(&state, &dm).await;
    assert!(matches!(node, EngineNode::ThisComputer(_)));
    let Err(refused) = make_daemon(&node, Network::Testnet).await else {
        panic!("a client was built behind the latch");
    };
    assert_eq!(refused, words);
    assert_eq!(core_target(&state, &dm).await.unwrap_err(), words);

    *state.node_address.write().await = Some("http://127.0.0.1:1".into());
    assert!(matches!(
        EngineNode::of(&state, &dm).await,
        EngineNode::Address(_)
    ));
    assert!(core_target(&state, &dm).await.is_ok(), "a typed address");
}

/// **Start mining pays the open wallet, and with none open it says so.**
/// The manager reads the wallet open when the start runs; with none, the
/// miner is not started and nothing is launched. Red edit: in
/// `start_mining`, start the miner without an open wallet.
#[tokio::test]
async fn start_mining_needs_an_open_wallet() {
    use shekyl_rpc_client::wire::node::{AutoWord, Threads};
    let dm = DaemonManager::new(
        Ok(std::env::temp_dir().join("shekyl-gui-none")),
        LaunchEnv::default(),
    );
    let refused = dm
        .start_mining(Threads::Auto(AutoWord::Auto))
        .await
        .expect_err("no wallet is open");
    assert_eq!(refused, payout::NO_WALLET_TO_PAY);
    assert!(dm.launched().is_empty());
    assert!(
        !dm.runs_the_saved_settings().await,
        "no node runs, so no setting is applied"
    );
}

/// **Two settings changed at once both stick** (finding on #73). A Tor fix
/// and the clearnet listener are changed from two tasks at once, many
/// times over; every change is read, made and saved under one lock, so
/// neither saves a copy that undoes the other, in memory or on disk. A
/// change that leaves the settings as they were saves nothing. Red edit:
/// in `change_config`, read the settings under a read lock and save under
/// a separate write (the old `config()` then `update_config`); one task's
/// last change is then lost.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_settings_changed_at_once_both_stick() {
    let dir = std::env::temp_dir().join(format!(
        "shekyl-gui-settings-{}-{}",
        std::process::id(),
        crate::engine_session::test_password()
    ));
    let dm = DaemonManager::new(Ok(dir.clone()), LaunchEnv::default());
    let fix = crate::tor_fix::TorFix::ClearnetOnly;
    let tor = {
        let dm = Arc::clone(&dm);
        let fix = fix.clone();
        tokio::spawn(async move {
            for i in 0..200 {
                let set = (i % 2 == 1).then(|| fix.clone());
                dm.change_config(|c| c.tor_fix = set).await.unwrap();
                tokio::task::yield_now().await;
            }
        })
    };
    let inbound = {
        let dm = Arc::clone(&dm);
        tokio::spawn(async move {
            for i in 0..200 {
                dm.change_config(|c| c.clearnet_inbound = i % 2 == 1)
                    .await
                    .unwrap();
                tokio::task::yield_now().await;
            }
        })
    };
    tor.await.unwrap();
    inbound.await.unwrap();

    let kept = dm.config().await;
    assert_eq!(kept.tor_fix, Some(fix.clone()), "{kept:?}");
    assert!(kept.clearnet_inbound, "{kept:?}");
    assert_eq!(DaemonConfig::load(&dir), kept, "the file holds both");

    assert!(
        !dm.change_config(|c| c.clearnet_inbound = true)
            .await
            .unwrap(),
        "no change, nothing saved"
    );
    drop(std::fs::remove_dir_all(&dir));
}

/// **Without a settings directory the node has nowhere to keep its
/// settings, so it is not started, and a settings change is refused, both
/// in words that say why**: an account with no home (core rule 83 IG-13: no
/// home, no default paths), and a per-account root the app's start could
/// not make with its protections, which nothing then makes another way.
/// Red edits: in `launch_locked`, fall back to `PathBuf::from(".")` when
/// there is no settings directory (the old Tauri fallback): a node is
/// launched; or read the settings directory off the root's path whatever
/// the start left: the root is made without its protections. The test
/// names its own socket directory (Unix) or instance (Windows), so it
/// never meets a node of the account it runs under; on Windows, the
/// platform whose root carries the owner-only and no-read-up protections,
/// a root not made is the case this pins.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_settings_dir_no_node_starts_and_no_setting_is_kept() {
    use crate::account_root::{AccountRoot, NO_SETTINGS_DIR};

    let dir = TestDir::new("no-home");
    let services = dir.0.join("services");
    std::fs::create_dir_all(&services).unwrap();
    let not_made = dir.0.join("root");
    for root in [
        AccountRoot::NoHome,
        AccountRoot::not_made_at(not_made.clone()),
    ] {
        let env = LaunchEnv {
            // A program that is never run: the test fails if anything launches.
            program: Some(dir.0.join("no-node-here")),
            tor_file: None,
            service_rendezvous: Some(services.clone()),
            socket_dir: cfg!(unix).then(|| dir.0.join("s")),
            instance: cfg!(windows).then(|| format!("nh-{}", std::process::id())),
            extra_args: Vec::new(),
        };
        let words = root.settings_dir().unwrap_err();
        if root == AccountRoot::NoHome {
            assert_eq!(words, NO_SETTINGS_DIR);
        }
        let dm = DaemonManager::new(root.settings_dir(), env);
        dm.start(NetworkType::Testnet).await;

        let status = dm.status().await;
        assert_eq!(status.state, "unavailable", "{status:?}");
        assert_eq!(status.message.as_deref(), Some(words.as_str()));
        assert!(dm.launched().is_empty(), "no node is launched");
        assert_eq!(
            dm.change_config(|c| c.clearnet_inbound = true).await,
            Err(words)
        );
        assert!(!not_made.exists(), "nothing made the root");
    }
}
