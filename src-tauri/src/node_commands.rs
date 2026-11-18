// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The app's commands for the wallet's own node: where it stands, its
//! settings, and the four acts a person can take on it
//! (`docs/design/GUI_SYNC_LANE.md` GS-2). The node's life is
//! [`crate::daemon_manager`]'s; these are its doors.

use std::sync::Arc;

use tauri::State;

use crate::daemon_manager::{DaemonConfig, DaemonManager, DaemonStatus, KEEP_RUNNING_NOT_YET};

#[tauri::command]
pub async fn daemon_status(dm: State<'_, Arc<DaemonManager>>) -> Result<DaemonStatus, String> {
    Ok(dm.status().await)
}

#[tauri::command]
pub async fn get_daemon_settings(
    dm: State<'_, Arc<DaemonManager>>,
) -> Result<DaemonConfig, String> {
    Ok(dm.config().await)
}

/// Set the node's settings. Keeping the node running after the wallet
/// closes is asked for here and answered in words: the node this wallet
/// starts lives as long as the wallet does, and a node that outlives it is
/// not built yet (GUI_SYNC_LANE.md `GS-Q2`).
#[tauri::command]
pub async fn set_daemon_settings(
    dm: State<'_, Arc<DaemonManager>>,
    keep_running_on_exit: Option<bool>,
    data_dir: Option<String>,
    clearnet_inbound: Option<bool>,
) -> Result<DaemonConfig, String> {
    if keep_running_on_exit == Some(true) {
        return Err(KEEP_RUNNING_NOT_YET.to_owned());
    }
    let mut config = dm.config().await;
    if let Some(v) = data_dir {
        config.data_dir = if v.is_empty() { None } else { Some(v) };
    }
    dm.update_config(config).await?;
    // The inbound clearnet listener is read at the node's start: changing
    // it starts the node again. A node that then refuses says why.
    if let Some(on) = clearnet_inbound {
        dm.set_clearnet_inbound(on).await?;
    }
    Ok(dm.config().await)
}

/// Stop the node of this account the wallet found running on its network,
/// and start the wallet's own. Returns once the wallet's node is ready, or
/// once a person has ended the wait for it.
#[tauri::command]
pub async fn replace_found_node(dm: State<'_, Arc<DaemonManager>>) -> Result<DaemonStatus, String> {
    dm.replace_found().await?;
    Ok(dm.status().await)
}

/// Start the wallet's node again after it failed to start or stopped, or
/// after a person ended a wait for it.
#[tauri::command]
pub async fn start_node(dm: State<'_, Arc<DaemonManager>>) -> Result<DaemonStatus, String> {
    dm.start_again().await;
    Ok(dm.status().await)
}

/// End the wallet's wait on a node it did not start: the found node that
/// has not gone yet, or the data directory it still holds. Nothing is
/// killed; the node is left idle, and `start_node` is the retry.
#[tauri::command]
pub async fn stop_waiting_for_node(dm: State<'_, Arc<DaemonManager>>) -> Result<(), String> {
    dm.stop_waiting().await
}

/// Kill the wallet's node. Only a person's press reaches this: every other
/// stop asks the node and waits for it.
#[tauri::command]
pub async fn force_stop_node(dm: State<'_, Arc<DaemonManager>>) -> Result<(), String> {
    dm.force_stop().await
}
