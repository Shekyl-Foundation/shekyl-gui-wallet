// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! What the wallet calls on its own node, held against what that node
//! admits (`docs/design/GUI_SYNC_LANE.md` GS-2, GS-4).
//!
//! The wallet and its engine read "this computer"'s node over its owner's
//! leg, which holds every grant (shekyl-core `RPC_CHANNEL.md` RT-W10 step
//! 5; Rick, 2026-11-18). A node address a person typed, a service node's
//! view listener among them, is read with a view listener's grants. Each
//! call is held against the authority of the leg it travels, asked of the
//! daemon's own admission code, never a copy of a grant set.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use shekyl_daemon_rpc::authority::Authority;
use shekyl_rpc_client::wire::{
    BlockHeaderShow, CurveShow, DaemonStatus, DaemonStop, MineStart, MineStatus, MineStop,
    ShardCoverage, Version,
};
use shekyl_rpc_types::method::Row;

/// Every daemon method the wallet calls itself, by the type name its call
/// sites spell.
const CALLS: &[(&str, Row)] = &[
    ("Version", Row::of::<Version>()),
    ("DaemonStatus", Row::of::<DaemonStatus>()),
    ("DaemonStop", Row::of::<DaemonStop>()),
    ("BlockHeaderShow", Row::of::<BlockHeaderShow>()),
    ("CurveShow", Row::of::<CurveShow>()),
    ("ShardCoverage", Row::of::<ShardCoverage>()),
    ("MineStatus", Row::of::<MineStatus>()),
    ("MineStart", Row::of::<MineStart>()),
    ("MineStop", Row::of::<MineStop>()),
];

/// The owner's leg admits every call the wallet makes itself, mining and
/// the stop among them. The switch for the armed test levers is off, as on
/// a node the wallet starts. Red edit: hold the calls against
/// `Authority::VIEW_LISTENER`, the leg the wallet used before GS-2; the
/// three mining calls and the stop are refused.
#[test]
fn the_owner_s_leg_admits_every_call_the_wallet_makes() {
    let owner = Authority::owner(false);
    for (name, row) in CALLS {
        assert!(
            owner.admit(row).is_ok(),
            "the owner's leg refuses {name} ({})",
            row.name
        );
    }
}

/// Every call the wallet engine makes (`shekyl_engine_core::DAEMON_METHODS`,
/// held to the engine's code by core's test) is admitted on both legs it
/// travels: the owner's leg of this computer's node, and a typed address's
/// view listener, a service node's among them.
#[test]
fn both_legs_admit_every_call_the_engine_makes() {
    let owner = Authority::owner(false);
    let view = Authority::VIEW_LISTENER;
    for row in shekyl_engine_core::DAEMON_METHODS {
        assert!(
            owner.admit(row).is_ok(),
            "this computer's node refuses the engine's {} on its owner's leg",
            row.name
        );
        assert!(
            view.admit(row).is_ok(),
            "a typed node refuses the engine's {}: it is outside the view \
             listener's grants",
            row.name
        );
    }
}

/// Every `.rs` file under `dir`, at any depth.
fn sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("a source directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}

/// The methods the wallet's source calls, read from its call sites under
/// `src` at any depth: each typed call names its method by the last path
/// segment of its turbofish. The generic `M` in `daemon_rpc` itself is not a
/// method, and this file, which spells the markers, is not read.
fn called_in_source(src: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in sources_under(src) {
        if path.file_name() == Some(std::ffi::OsStr::new("grant_reach_tests.rs")) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file");
        for marker in ["call::<", "call_within::<"] {
            for (at, _) in text.match_indices(marker) {
                let rest = &text[at + marker.len()..];
                let Some(end) = rest.find('>') else { continue };
                let name = rest[..end].rsplit("::").next().unwrap_or("").trim();
                let ident =
                    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if ident && name != "M" {
                    found.insert(name.to_owned());
                }
            }
        }
    }
    found
}

/// The list above is the call sites: a new `call::<T>` with no row here
/// fails, and so does a row whose last call site is gone.
#[test]
fn the_list_is_the_wallet_s_call_sites() {
    let listed: BTreeSet<String> = CALLS.iter().map(|(name, _)| (*name).to_owned()).collect();
    let called = called_in_source(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    assert_eq!(
        called, listed,
        "CALLS must name exactly the methods the wallet calls"
    );
}

/// The scan descends: a call site in a file under a subdirectory of `src`
/// is read like one at the top. Red edit: list only the top directory, as
/// the scan first did.
#[test]
fn the_scan_reads_call_sites_in_subdirectories() {
    let root = std::env::temp_dir().join(format!(
        "shekyl-gui-grant-scan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let deep = root.join("a").join("b");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(
        deep.join("nested.rs"),
        "async fn f() { daemon_rpc::call::<wire::PeerConnections>(u, p, w).await; }",
    )
    .unwrap();
    let found = called_in_source(&root);
    std::fs::remove_dir_all(&root).unwrap();
    assert!(found.contains("PeerConnections"), "{found:?}");
}
