// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Which daemon methods the wallet's own node lets it call.
//!
//! The wallet starts its node with one view listener and no admin listener
//! (`daemon_manager::sidecar_args`). Mining failed in silence at alpha.10
//! and alpha.11 because nothing compared what the wallet calls with what that
//! connection may call. This test asks the daemon's own admission check,
//! [`Authority::VIEW_LISTENER`], about every method the wallet calls and
//! every method its engine calls (`shekyl_engine_core::DAEMON_METHODS`:
//! version, status, fees, submission, the scan's block reads), so a call
//! the wallet's node would refuse is a red test, not a page that reads
//! nothing or a send that fails.
//!
//! A method the wallet calls that its node does not admit yet is named in
//! [`AWAITING`], with the core increment that will grant it. When GS-C3 (the
//! owning connection) lands, the authority here becomes the owner leg's and
//! the list empties.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use shekyl_daemon_rpc::authority::Authority;
use shekyl_rpc_client::wire::{
    BlockHeaderShow, CurveShow, DaemonStatus, MineStart, MineStatus, MineStop, ShardCoverage,
    Version,
};
use shekyl_rpc_types::method::Row;

/// Every daemon method the wallet calls through `daemon_rpc::call`, by the
/// type name its call sites spell.
const CALLS: &[(&str, Row)] = &[
    ("Version", Row::of::<Version>()),
    ("DaemonStatus", Row::of::<DaemonStatus>()),
    ("BlockHeaderShow", Row::of::<BlockHeaderShow>()),
    ("CurveShow", Row::of::<CurveShow>()),
    ("ShardCoverage", Row::of::<ShardCoverage>()),
    ("MineStatus", Row::of::<MineStatus>()),
    ("MineStart", Row::of::<MineStart>()),
    ("MineStop", Row::of::<MineStop>()),
];

/// Methods the wallet or its engine calls that its own node does not admit
/// yet, by wire name, each with the core increment that will grant it
/// (`docs/design/GUI_SYNC_LANE.md` §5).
const AWAITING: &[(&str, &str)] = &[
    ("mine_status", "GS-C3"),
    ("mine_start", "GS-C3"),
    ("mine_stop", "GS-C3"),
];

/// Every method that reaches the wallet's own node: the GUI's own calls,
/// and the engine's (`shekyl_engine_core::DAEMON_METHODS`, held to the
/// engine's code by core's test), which go over the same connection.
fn reaching_the_node() -> Vec<(&'static str, &'static Row)> {
    CALLS
        .iter()
        .map(|(_, row)| ("the wallet", row))
        .chain(
            shekyl_engine_core::DAEMON_METHODS
                .iter()
                .map(|row| ("the engine", row)),
        )
        .collect()
}

/// Every call is admitted by the wallet's own node, or is named as waiting
/// on an increment; and every waiting entry is still refused, so an entry
/// whose increment has landed is struck rather than left to hide a gap.
#[test]
fn every_call_is_admitted_or_awaits_a_named_increment() {
    let node = Authority::VIEW_LISTENER;
    let calls = reaching_the_node();
    for (caller, row) in &calls {
        let admitted = node.admit(row).is_ok();
        match AWAITING.iter().find(|(waiting, _)| *waiting == row.name) {
            Some((_, increment)) => assert!(
                !admitted,
                "{} ({caller}) is admitted now: strike it from AWAITING ({increment})",
                row.name
            ),
            None => assert!(
                admitted,
                "the wallet's own node refuses {} ({caller}): it is outside the view \
                 listener's grants, and no increment is named for it in AWAITING",
                row.name
            ),
        }
    }
    for (waiting, increment) in AWAITING {
        assert!(
            calls.iter().any(|(_, row)| row.name == *waiting),
            "AWAITING names {waiting} ({increment}), which neither the wallet nor its engine calls"
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
