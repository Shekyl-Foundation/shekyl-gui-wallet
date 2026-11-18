// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The daemon this wallet talks to: where it points, what pointing there
//! discloses, and how the node is doing.
//!
//! Carved out of `commands.rs` when the disclosure surface pushed that file
//! back over its ratchet ceiling. The three belong together: they are the
//! same question asked three ways, and every one of them reads
//! `AppState::node_address`. Splitting by feature is what the ratchet asks
//! for instead of a raised ceiling
//! (`.cursor/rules/27-composition-decomposition.mdc`).

use serde::Serialize;
use tauri::State;

use crate::daemon_rpc;
use crate::state::{AppState, NetworkType};
use shekyl_rpc_client::wire::block::{BlockHeader, BlockSelect, HeaderSelect};
use shekyl_rpc_client::wire::chain::{Curve, CurveSelect};
use shekyl_rpc_client::wire::daemon::DaemonStatus as Status;
use shekyl_rpc_client::wire::shared::Selector;
use shekyl_rpc_client::wire::{BlockHeaderShow, CurveShow, DaemonStatus};
use shekyl_rpc_types::kinds::NoParams;
use shekyl_units::AtomicUnitsString;

/// What setting a daemon URL exposes, returned to the panel that set it.
///
/// `warnings` is empty for a daemon on this machine, which is the default
/// and the supported configuration. A daemon that is not loopback carries
/// the §1 operator statement of shekyl-core's `RPC_TRANSPORT_POSTURE.md`:
/// whoever runs that daemon sees which blocks this wallet asks for and what
/// it broadcasts. The wallet does not refuse it — the operator may well own
/// the far end — it says what is true, where the address was typed
/// (RT-W7: discouragement where it is consumed).
#[derive(Debug, Clone, Serialize)]
pub struct DaemonConnection {
    /// The node address a person typed; `None`: this computer's node, the
    /// one the wallet starts, which has no address to show.
    pub url: Option<String>,
    /// Disclosures for a typed address; never an assurance, only ever
    /// warnings. None for this computer's node.
    pub warnings: Vec<String>,
    /// The network the wallet is on: the one source for the page, since
    /// the node's own status follows a switch only once it has restarted.
    pub network: &'static str,
}

/// What a change of node address says while a wallet is open. The open
/// wallet's engine keeps the node it was opened on for its scans and its
/// sends, so a change under it would have the status read from one node
/// and the wallet from another.
pub(crate) const NODE_CHANGE_WAITS_FOR_CLOSE: &str =
    "Close the wallet before changing the node it uses: an open wallet keeps the node it was \
     opened on.";

/// The disclosures for a daemon URL the wallet is about to use.
///
/// This wallet dials the daemon directly and has no proxy, so the
/// network-path half of the disclosure is judged with no proxy configured
/// and `AlwaysRemote` is the honest resolution: there is no proxy for a
/// hostname to leak *past*, and claiming a local-resolver leak would assert
/// a lookup no configured transport performs.
fn daemon_disclosures(url: &str) -> Vec<String> {
    shekyl_rpc_transport::network_posture::daemon_disclosures(
        "daemon URL",
        url,
        None,
        shekyl_rpc_transport::network_posture::ProxyResolution::AlwaysRemote,
    )
    .into_iter()
    .collect()
}

/// Point the wallet at `network`, and at `url` when a person typed one,
/// otherwise at this computer's node. The GUI's own node follows the
/// network: a change relaunches it on the new network. With a wallet open,
/// a change of network or of node is refused: the wallet stays on its
/// network, and its engine on the node it was opened on.
#[tauri::command]
pub async fn set_daemon_connection(
    state: State<'_, AppState>,
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
    network: String,
    url: Option<String>,
) -> Result<DaemonConnection, String> {
    let net = NetworkType::from_name(&network)
        .ok_or("Invalid network: must be mainnet, testnet, or stagenet")?;
    point_at(&state, &dm, net, url).await
}

/// [`set_daemon_connection`]'s work: the typed address (or none), the
/// network, kept for the next launch, and the node relaunched on it.
pub(crate) async fn point_at(
    state: &AppState,
    dm: &std::sync::Arc<crate::daemon_manager::DaemonManager>,
    net: NetworkType,
    url: Option<String>,
) -> Result<DaemonConnection, String> {
    let url = url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    let warnings = url.as_deref().map(daemon_disclosures).unwrap_or_default();
    // The session guard, from the check to the last write: no wallet opens
    // between them on the network or the node being replaced.
    let session = state.engine.lock().await;
    if session.is_open() {
        crate::daemon_manager::may_switch(session.open_network(), net)?;
        if url != state.node_address().await {
            return Err(NODE_CHANGE_WAITS_FOR_CLOSE.to_owned());
        }
        // The node the open wallet already uses: nothing changes.
        return Ok(DaemonConnection {
            url,
            warnings,
            network: net.as_str(),
        });
    }
    *state.node_address.write().await = url.clone();
    *state.network.write().await = net;
    // The node starts on this network at the next launch too.
    crate::gui_config::save_network(net);
    // The relaunch waits for the running node to be gone and the new one to
    // start; the page does not.
    let dm = std::sync::Arc::clone(dm);
    tauri::async_runtime::spawn(async move { dm.relaunch(net).await });
    drop(session);
    Ok(DaemonConnection {
        url,
        warnings,
        network: net.as_str(),
    })
}

/// Point the wallet at the service node on this computer: its view
/// listener, as the service's rendezvous record names it, becomes the typed
/// address (Rick, 2026-11-18). Until wallets reach a service over the
/// channel (RT-W12), that listener is how a wallet on this computer reads
/// it.
#[tauri::command]
pub async fn use_service_node(
    state: State<'_, AppState>,
    dm: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<DaemonConnection, String> {
    point_at_service(&state, &dm).await
}

/// [`use_service_node`]'s work.
pub(crate) async fn point_at_service(
    state: &AppState,
    dm: &std::sync::Arc<crate::daemon_manager::DaemonManager>,
) -> Result<DaemonConnection, String> {
    let view = dm
        .service_view()
        .await
        .ok_or("No service node with a view listener runs on this network.")?;
    let net = *state.network.read().await;
    point_at(state, dm, net, Some(format!("http://{view}"))).await
}

/// The disclosures for the daemon URL already in effect, so a wallet that
/// starts with a non-loopback daemon says so too — not only the session in
/// which it was typed.
#[tauri::command]
pub async fn daemon_connection_disclosures(
    state: State<'_, AppState>,
) -> Result<DaemonConnection, String> {
    let url = state.node_address().await;
    let warnings = url.as_deref().map(daemon_disclosures).unwrap_or_default();
    Ok(DaemonConnection {
        url,
        warnings,
        network: state.network.read().await.as_str(),
    })
}

/// The chain as the node reports it (`daemon_status`), with the tip's
/// header (`block_header_show`) and curve tree (`curve_show`). Field names
/// follow the 4.0 wire (shekyl-core `INPUT_GRAMMAR.md`, the `daemon_status`
/// table). A read the node did not answer is `null`, never zero.
#[derive(Debug, Serialize)]
pub struct ChainHealth {
    /// The wire's `height` unchanged: the tip's own block number (genesis
    /// is 0), shown as "Block number".
    pub height: u64,
    pub hash: String,
    /// A 128-bit value, as a decimal string.
    pub difficulty: String,
    pub tx_count: u64,
    /// `null` while the node's pool is retired.
    pub txpool_count: Option<u64>,
    pub synchronized: bool,
    /// Coins emitted so far.
    pub emission: AtomicUnitsString,
    /// The emission multiplier, in millionths.
    pub emission_multiplier: u64,
    /// The burn rate, in millionths.
    pub burn: u64,
    pub burn_total: AtomicUnitsString,
    /// The stakers' effective share of emission, in millionths.
    pub stake_share: u64,
    /// The chain's block spacing, seconds.
    pub block_spacing: u64,
    pub network: String,
    pub last_block: Option<LastBlock>,
    pub curve_tree: Option<CurveTree>,
}

/// The tip's header.
#[derive(Debug, Serialize)]
pub struct LastBlock {
    pub reward: AtomicUnitsString,
    /// Unix seconds.
    pub time: u64,
    pub hash: String,
    /// Bytes.
    pub weight: u64,
}

/// The curve tree at the tip.
#[derive(Debug, Serialize)]
pub struct CurveTree {
    pub root: String,
    pub layers: u64,
    pub leaf_count: u64,
}

#[tauri::command]
pub async fn get_chain_health(
    state: State<'_, AppState>,
    node: State<'_, std::sync::Arc<crate::daemon_manager::DaemonManager>>,
) -> Result<ChainHealth, String> {
    let network = *state.network.read().await;

    let status =
        daemon_rpc::node_call::<DaemonStatus>(&state, &node, NoParams, "the node's status").await?;
    let tip = HeaderSelect(BlockSelect {
        select: Selector::Tip,
        pow: false,
        prune: None,
    });
    let header = daemon_rpc::node_call::<BlockHeaderShow>(&state, &node, tip, "the last block")
        .await
        .ok();
    let curve = daemon_rpc::node_call::<CurveShow>(
        &state,
        &node,
        CurveSelect(Selector::Tip),
        "the curve tree",
    )
    .await
    .ok();
    chain_health(status, header, curve, network.as_str())
}

/// The words for a node that withholds its chain part from this wallet.
pub(crate) const CHAIN_WITHHELD: &str = "This node does not let this wallet see the chain.";

/// The chain read from the node's three replies. The header and the curve
/// are `None` when the node did not answer them, and stay `null` on the
/// page. `daemon_status`'s chain part is shown whole to a connection
/// holding the `chain` grant and is absent otherwise (4.2); the wallet's
/// own node holds it, and a node that withholds it is said to, never read
/// as a chain of zeros. A withheld pool part is a `null` pool count.
fn chain_health(
    status: Status,
    header: Option<BlockHeader>,
    curve: Option<Curve>,
    network: &str,
) -> Result<ChainHealth, String> {
    let withheld = || CHAIN_WITHHELD.to_owned();
    Ok(ChainHealth {
        height: status.height.0,
        hash: status.hash.to_string(),
        difficulty: status
            .difficulty
            .shown()
            .ok_or_else(withheld)?
            .get()
            .to_string(),
        tx_count: status.tx_count.shown().ok_or_else(withheld)?.0,
        txpool_count: status.txpool_count.shown().and_then(|c| c.0).map(|c| c.0),
        synchronized: status.synchronized,
        emission: status.emission.shown().ok_or_else(withheld)?,
        emission_multiplier: status.emission_multiplier.shown().ok_or_else(withheld)?.0,
        burn: status.burn.shown().ok_or_else(withheld)?.0,
        burn_total: status.burn_total.shown().ok_or_else(withheld)?,
        stake_share: status.stake_share.shown().ok_or_else(withheld)?.0,
        block_spacing: status.block_spacing.shown().ok_or_else(withheld)?.0,
        network: network.into(),
        last_block: header.map(|h| LastBlock {
            reward: h.reward,
            time: h.time.0,
            hash: h.hash.to_string(),
            weight: h.weight.0,
        }),
        curve_tree: curve.map(|c| CurveTree {
            root: c.root.to_string(),
            layers: c.layers.0,
            leaf_count: c.leaf_count.0,
        }),
    })
}

#[cfg(test)]
mod chain_health_tests {
    use super::*;
    use crate::wire_vectors::reply;

    /// Core's own replies become the page's chain read, each figure from
    /// its wire field (`INPUT_GRAMMAR.md`'s `daemon_status` table). The
    /// wallet asks for the tip's header without its proof of work.
    #[test]
    fn the_chain_read_is_built_from_core_s_replies() {
        let health = chain_health(
            reply("daemon_status"),
            Some(reply("block_header_show.plain")),
            Some(reply("curve_show")),
            "testnet",
        )
        .expect("the vector shows the chain part");
        let v = serde_json::to_value(&health).unwrap();
        // The tip's own number, unchanged: tip 1199 shows 1,199.
        assert_eq!(v["height"], 1199);
        assert!(v.get("coinbase_lock_window").is_none(), "not the node's");
        assert!(v.get("target_height").is_none(), "nothing reads it");
        assert_eq!(
            v["difficulty"],
            (0x2a3f1c_u64).to_string(),
            "the hex128, as decimal"
        );
        assert_eq!(v["emission"], "50212500000000");
        assert_eq!(v["emission_multiplier"], 1_000_000);
        assert_eq!(v["burn"], 50_000);
        assert_eq!(v["burn_total"], "1250000000");
        assert_eq!(v["stake_share"], 300_000);
        assert_eq!(v["txpool_count"], 2);
        assert_eq!(v["block_spacing"], 120);
        assert!(v["last_block"]["reward"].is_string());
        assert!(v["curve_tree"]["layers"].is_u64());
        assert_eq!(v["network"], "testnet");
    }

    /// A header or curve the node did not answer is `null`, never zeros.
    #[test]
    fn an_unanswered_header_or_curve_is_null() {
        let v = serde_json::to_value(
            chain_health(reply("daemon_status"), None, None, "mainnet").expect("chain shown"),
        )
        .unwrap();
        assert!(v["last_block"].is_null());
        assert!(v["curve_tree"].is_null());
    }

    /// A node that withholds the chain part (a connection without the
    /// `chain` grant) is said to, never read as a chain of zeros; a withheld
    /// pool part is a `null` pool count. Red edit: `unwrap_or_default()` on
    /// a chain field.
    #[test]
    fn a_withheld_chain_part_is_refused_in_words_and_a_withheld_pool_is_null() {
        let mut value: serde_json::Value = reply("daemon_status");
        let members = value.as_object_mut().unwrap();
        for key in [
            "difficulty",
            "difficulty_cumulative",
            "block_spacing",
            "tx_count",
            "block_weight_max",
            "block_weight_median",
            "time_adjusted",
            "emission",
            "emission_multiplier",
            "burn",
            "burn_total",
            "stake_share",
        ] {
            members.remove(key);
        }
        let status: Status = serde_json::from_value(value).expect("the chain part withheld");
        assert_eq!(
            chain_health(status, None, None, "mainnet").expect_err("withheld"),
            CHAIN_WITHHELD
        );

        let mut value: serde_json::Value = reply("daemon_status");
        value.as_object_mut().unwrap().remove("txpool_count");
        let status: Status = serde_json::from_value(value).expect("the pool part withheld");
        let v = serde_json::to_value(chain_health(status, None, None, "mainnet").unwrap()).unwrap();
        assert!(v["txpool_count"].is_null());
    }
}
