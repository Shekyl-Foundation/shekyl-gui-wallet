// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The node's Tor state, in core's words (`docs/design/GUI_SYNC_LANE.md`
//! GS-3; shekyl-core `TOR_STATUS_CRATE.md`).
//!
//! The wallet reads `daemon_status`'s Status part and shows
//! `shekyl_tor_status::render` verbatim: one definition and one renderer,
//! the same sentences the node's log and the CLI say. The wallet rewords
//! nothing and keeps no copy of a field.
//!
//! The Status part is shown to the `status` grant, which the wallet holds
//! on the node it starts (the owner's leg). A node address a person typed
//! may withhold it; the wallet then says it cannot see that node's Tor
//! state, never an empty posture.

use serde::Serialize;
use shekyl_rpc_client::wire::daemon::DaemonStatus as Status;
use shekyl_rpc_client::wire::DaemonStatus;
use shekyl_rpc_types::kinds::NoParams;
use tauri::State;

use crate::daemon_rpc;
use crate::state::AppState;

/// The words for a node that does not show this wallet its Status part.
pub(crate) const TOR_WITHHELD: &str = "This node does not let this wallet see its Tor state.";

/// The node's Tor state for the page.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TorRead {
    /// Whether the node showed its Tor state. `false`: `text` says it is
    /// withheld.
    pub shown: bool,
    /// Core's rendering, verbatim; or the withheld words.
    pub text: String,
}

#[tauri::command]
pub async fn get_tor_status(
    state: State<'_, AppState>,
    node: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<TorRead, String> {
    let status =
        daemon_rpc::node_call::<DaemonStatus>(&state, &node, NoParams, "the node's Tor state")
            .await?;
    Ok(tor_read(&status))
}

/// The Tor read from a `daemon_status` reply. The data directory is not
/// named: the rendering names it only for a caller the node told it to.
fn tor_read(status: &Status) -> TorRead {
    match status.tor_status() {
        Some(tor) => TorRead {
            shown: true,
            text: shekyl_tor_status::render(&tor, None),
        },
        None => TorRead {
            shown: false,
            text: TOR_WITHHELD.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire_vectors::reply;

    /// The five fields of the Status part's Tor state.
    const TOR_FIELDS: [&str; 5] = [
        "posture",
        "origination",
        "sync_connector",
        "tor_quiet",
        "tor",
    ];

    /// The page's text is core's rendering of the reply, character for
    /// character. Red edit: build the text here from the fields.
    #[test]
    fn the_text_is_core_s_rendering_verbatim() {
        let status: Status = reply("daemon_status");
        let tor = status.tor_status().expect("the vector shows Status");
        let read = tor_read(&status);
        assert!(read.shown);
        assert_eq!(read.text, shekyl_tor_status::render(&tor, None));
        assert!(!read.text.is_empty());
    }

    /// The quiet line reaches the page: a node that knows peers and has
    /// reached none through Tor says so, in core's sentence.
    #[test]
    fn the_quiet_line_is_in_the_text() {
        let mut value: serde_json::Value = reply("daemon_status");
        value["tor_quiet"] = serde_json::json!({"unreached": {"age": 130}});
        let status: Status = serde_json::from_value(value).expect("a quiet node");
        let tor = status.tor_status().expect("shown");
        assert!(tor.tor_quiet.is_some());
        let quiet = tor_read(&status).text;
        let mut value: serde_json::Value = reply("daemon_status");
        value["tor_quiet"] = serde_json::Value::Null;
        let status: Status = serde_json::from_value(value).expect("a node with no quiet line");
        assert_ne!(quiet, tor_read(&status).text, "the quiet line is said");
    }

    /// A node that withholds the Status part is said to: the part is absent
    /// whole, and no field stands in for it. Red edit: render a default
    /// status for a withheld part.
    #[test]
    fn a_withheld_status_part_is_said_never_an_empty_posture() {
        let mut value: serde_json::Value = reply("daemon_status");
        let members = value.as_object_mut().unwrap();
        // The whole Status part, as a connection without `status` sees it.
        for key in status_part_keys() {
            members.remove(key);
        }
        let status: Status = serde_json::from_value(value).expect("the Status part withheld");
        assert_eq!(
            tor_read(&status),
            TorRead {
                shown: false,
                text: TOR_WITHHELD.to_owned()
            }
        );
    }

    /// The Status part's keys in the vector: the Tor fields, and the rest
    /// of the part they travel with.
    fn status_part_keys() -> Vec<&'static str> {
        let mut keys = TOR_FIELDS.to_vec();
        keys.extend(STATUS_PART_REST);
        keys
    }

    /// The Status part's other fields (shekyl-core `wire/daemon.rs`, the
    /// part shown to `status`).
    const STATUS_PART_REST: &[&str] = &[
        "started",
        "disk_free",
        "data_size",
        "randomx_cache_places",
        "revision",
        "peer_outbound_count",
        "peer_inbound_count",
        "alt_count",
        "rpc_connections",
        "txpool_retired",
    ];
}
