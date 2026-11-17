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
use shekyl_tor_status::{Carriage, Clearnet, Fix, Origination, TorSetting, TorStatus};
use tauri::State;

use crate::daemon_rpc;
use crate::state::AppState;

/// The words for a node that does not show this wallet its Status part.
pub(crate) const TOR_WITHHELD: &str = "This node does not let this wallet see its Tor state.";

/// What happens to a transaction this wallet sends through the node now:
/// the one reading the header, the send review and the pending list share.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Sends {
    /// It leaves on a session that hides this node's address.
    Hidden,
    /// It waits on the node until Tor carries it. It does not go out over
    /// clearnet.
    Held,
    /// It goes out over the open network: Tor is off and clearnet is on.
    OpenNetwork,
    /// The node is offline: nothing is sent.
    Offline,
    /// The node reports something this build does not know. Nothing is
    /// promised about a send.
    Unknown,
    /// The node does not show its Tor state. Nothing is promised either.
    Withheld,
}

/// What the node's Tor status means for a send.
///
/// Hidden is core's one accessor (`own_tx_hides_address`), which fails
/// closed, and never anything else. One case is read beside it: while the
/// link carries nothing (`carriage` is `gone`) the relay's verdict lags a
/// Tor that ended, and core's renderer speaks the held sentence. The
/// wallet says what the renderer says, so its header and its review never
/// disagree with the text beside them.
fn sends(tor: &TorStatus) -> Sends {
    let link_gone = tor
        .tor
        .as_ref()
        .is_some_and(|managed| managed.carriage == Carriage::Gone);
    if tor.own_tx_hides_address() {
        return if link_gone {
            Sends::Held
        } else {
            Sends::Hidden
        };
    }
    let settings_known = matches!(
        tor.posture.tor,
        TorSetting::Managed | TorSetting::Operator | TorSetting::Off
    ) && matches!(tor.posture.clearnet, Clearnet::On | Clearnet::Off);
    match &tor.origination {
        Origination::Connecting | Origination::Rebuilding if settings_known => Sends::Held,
        Origination::ClearnetOnly if settings_known => Sends::OpenNetwork,
        Origination::Offline if settings_known => Sends::Offline,
        _ => Sends::Unknown,
    }
}

/// A fix the node's read offers, for Settings → Network.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct OfferedFix {
    /// Core's keyword for it.
    pub keyword: String,
}

/// The node's Tor state for the page.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TorRead {
    /// Whether the node showed its Tor state. `false`: `text` says it is
    /// withheld.
    pub shown: bool,
    /// Core's rendering, verbatim; or the withheld words.
    pub text: String,
    /// What happens to a send now.
    pub sends: Sends,
    /// The fixes the read offers, in its order. Empty unless the node's
    /// managed Tor has an install cause.
    pub fixes: Vec<OfferedFix>,
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
            sends: sends(&tor),
            fixes: tor
                .tor
                .iter()
                .flat_map(|managed| &managed.fix)
                .filter(|fix| !matches!(fix, Fix::Unknown { .. }))
                .map(|fix| OfferedFix {
                    keyword: fix.keyword().to_owned(),
                })
                .collect(),
        },
        None => TorRead {
            shown: false,
            text: TOR_WITHHELD.to_owned(),
            sends: Sends::Withheld,
            fixes: Vec::new(),
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

    fn status(change: impl FnOnce(&mut serde_json::Value)) -> TorStatus {
        let mut value: serde_json::Value = reply("daemon_status");
        change(&mut value);
        let status: Status = serde_json::from_value(value).expect("a daemon_status reply");
        status.tor_status().expect("the Status part is shown")
    }

    /// One reading for the header, the review and the pending list. Hidden
    /// is core's accessor alone; held promises no clearnet and is said only
    /// for `connecting` and `rebuilding`; a keyword this build does not
    /// know promises nothing. Red edit: fold `Unknown` into `Held` (the
    /// review would promise that a send stays off the open network on a
    /// node whose state the wallet cannot read).
    #[test]
    fn a_send_is_classified_once_and_an_unknown_promises_nothing() {
        let of = |origination: &str, tor: &str, clearnet: &str| {
            sends(&status(|v| {
                v["origination"] = origination.into();
                v["posture"] = serde_json::json!({"tor": tor, "clearnet": clearnet});
                v["tor"] = serde_json::Value::Null;
            }))
        };
        assert_eq!(of("address-hiding", "operator", "on"), Sends::Hidden);
        assert_eq!(of("connecting", "operator", "on"), Sends::Held);
        assert_eq!(of("rebuilding", "operator", "off"), Sends::Held);
        assert_eq!(of("clearnet-only", "off", "on"), Sends::OpenNetwork);
        assert_eq!(of("offline", "off", "off"), Sends::Offline);
        assert_eq!(of("a-word-from-later", "operator", "on"), Sends::Unknown);
        assert_eq!(
            of("connecting", "a-setting-from-later", "on"),
            Sends::Unknown
        );
        // Core's accessor fails closed on Tor off; so does this.
        assert_eq!(of("address-hiding", "off", "on"), Sends::Unknown);
        for origination in [
            "address-hiding",
            "connecting",
            "rebuilding",
            "clearnet-only",
            "offline",
        ] {
            let tor = status(|v| {
                v["origination"] = origination.into();
                v["posture"] = serde_json::json!({"tor": "operator", "clearnet": "on"});
                v["tor"] = serde_json::Value::Null;
            });
            assert_eq!(
                sends(&tor) == Sends::Hidden,
                tor.own_tx_hides_address(),
                "{origination}: hidden is core's accessor"
            );
        }
    }

    /// While the managed link carries nothing, core's renderer speaks the
    /// held sentence even where the relay's verdict still reads
    /// `address-hiding`. The wallet's reading is the renderer's, so the
    /// header is never silent beside a text that says holding.
    #[test]
    fn a_link_that_carries_nothing_reads_as_held_as_the_renderer_says() {
        let gone = status(|v| {
            v["origination"] = "address-hiding".into();
            v["posture"] = serde_json::json!({"tor": "managed", "clearnet": "on"});
            v["tor"]["carriage"] = "gone".into();
        });
        assert!(gone.own_tx_hides_address());
        assert_eq!(sends(&gone), Sends::Held);
        assert!(
            shekyl_tor_status::render(&gone, None).contains("holding its own transactions"),
            "{}",
            shekyl_tor_status::render(&gone, None)
        );
        let carries = status(|v| {
            v["origination"] = "address-hiding".into();
            v["posture"] = serde_json::json!({"tor": "managed", "clearnet": "on"});
            v["tor"]["carriage"] = "carries".into();
        });
        assert_eq!(sends(&carries), Sends::Hidden);
    }

    /// The fixes are the read's, in its order, by core's keywords.
    #[test]
    fn the_offered_fixes_are_the_read_s_in_its_order() {
        let mut value: serde_json::Value = reply("daemon_status");
        value["posture"] = serde_json::json!({"tor": "managed", "clearnet": "off"});
        value["tor"]["cause"] = serde_json::json!({"missing": null});
        value["tor"]["fix"] = serde_json::json!([
            "restore-managed",
            "operator",
            "clearnet-on",
            "clearnet-only"
        ]);
        let status: Status = serde_json::from_value(value).expect("a node with no Tor");
        let keywords: Vec<String> = tor_read(&status)
            .fixes
            .into_iter()
            .map(|f| f.keyword)
            .collect();
        assert_eq!(
            keywords,
            [
                "restore-managed",
                "operator",
                "clearnet-on",
                "clearnet-only"
            ]
        );
    }

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
                text: TOR_WITHHELD.to_owned(),
                sends: Sends::Withheld,
                fixes: Vec::new(),
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
