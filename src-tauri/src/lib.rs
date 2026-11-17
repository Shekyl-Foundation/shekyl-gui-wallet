// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::sync::Arc;

use tauri::Manager;

mod balance;
mod chain_constants;
mod clipboard;
mod commands;
mod contract_error;
mod daemon_connection;
mod daemon_manager;
mod daemon_rpc;
mod drain_balance;
mod engine_daemon;
mod engine_errors;
mod engine_session;
mod features;
#[cfg(test)]
mod grant_reach_tests;
mod gui_config;
mod lifecycle;
#[cfg(test)]
mod live_daemon_tests;
#[cfg(test)]
mod live_own_node_tests;
mod message_signing;
mod mining;
#[cfg(feature = "multisig")]
mod multisig;
mod node_commands;
mod node_launch;
mod proofs;
mod receiving;
mod seed_phrase;
mod send;
mod shard_coverage;
mod shard_visual;
mod staking_actions;
mod staking_view;
mod state;
mod tor_status;
mod transfer_history;
mod transfer_lookup;
mod tx_journal;
mod validate;
mod wallet_care;
mod wallet_name;
#[cfg(test)]
mod wire_vectors;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state::AppState::new())
        .manage(clipboard::ClipboardOwner::new())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let resource_dir = app.path().resource_dir();
            if let Err(e) = &resource_dir {
                // Without the resource directory the wallet cannot name its
                // Tor; the daemon then searches for one and announces what
                // it found.
                tracing::warn!("no resource directory for the bundled Tor: {e}");
            }
            let env = daemon_manager::LaunchEnv::bundled(resource_dir.ok().as_deref());
            let gate = app.state::<state::AppState>().node_gate.clone();
            let dm = daemon_manager::DaemonManager::new(config_dir, env, gate);
            app.manage(dm.clone());

            tauri::async_runtime::spawn(async move {
                dm.start(crate::state::NetworkType::default()).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Daemon / chain
            commands::get_wallet_status,
            daemon_connection::get_chain_health,
            daemon_connection::set_daemon_connection,
            daemon_connection::daemon_connection_disclosures,
            commands::get_pqc_status,
            commands::get_security_status,
            features::get_feature_flags,
            clipboard::copy_to_clipboard,
            clipboard::clear_clipboard,
            // Mining
            chain_constants::get_chain_constants,
            mining::get_mining_status,
            mining::start_mining_cmd,
            mining::stop_mining_cmd,
            // Wallet startup
            commands::check_wallet_files,
            commands::ensure_wallet_dir,
            commands::set_wallet_dir,
            commands::reset_wallet_dir,
            commands::get_wallet_dir,
            // Wallet lifecycle
            lifecycle::create_wallet,
            lifecycle::open_wallet,
            lifecycle::close_wallet,
            lifecycle::restore_wallet,
            commands::get_staker_status,
            commands::stake,
            staking_actions::stake_in,
            staking_actions::drain,
            staking_actions::unstake,
            staking_actions::collect_unstaked,
            proofs::get_tx_proof,
            proofs::check_tx_proof,
            proofs::get_reserve_proof,
            proofs::check_reserve_proof,
            message_signing::sign_message,
            message_signing::verify_message,
            wallet_care::refresh,
            wallet_care::rescan_blockchain,
            wallet_care::change_password,
            tx_journal::set_tx_note,
            tx_journal::get_tx_note,
            tx_journal::abandon_tx,
            transfer_lookup::get_transfer_by_id,
            // Wallet data
            commands::get_balance,
            commands::get_drain_balance,
            commands::get_staking_view,
            commands::get_primary_address,
            send::get_default_fee_priority,
            send::build_pending_tx,
            send::submit_pending_tx,
            send::discard_pending_tx,
            transfer_history::get_transfers,
            // Receiving: payment requests and the shekyl: URI (contract names).
            // `make_uri` is the contract's freeform composer; this GUI shows
            // the stored link from the list instead, so it is not registered.
            receiving::create_payment_request,
            receiving::list_payment_requests,
            receiving::parse_uri,
            // Shard identity preview (pre-archival beta)
            shard_visual::list_shard_preview_fixtures,
            shard_visual::render_shard_preview,
            // Shards page: daemon coverage (app shell) + the contract's
            // get_shard_view, drawn here (SHARD_VIEW_FETCH.md SV-D)
            shard_coverage::list_shards,
            shard_coverage::get_shard_view,
            // PQC Multisig — only under the `multisig` cargo feature (see multisig.rs)
            #[cfg(feature = "multisig")]
            multisig::create_multisig_group,
            #[cfg(feature = "multisig")]
            multisig::get_multisig_info,
            #[cfg(feature = "multisig")]
            multisig::sign_multisig_partial,
            #[cfg(feature = "multisig")]
            multisig::export_group_descriptor,
            #[cfg(feature = "multisig")]
            multisig::import_group_descriptor,
            #[cfg(feature = "multisig")]
            multisig::export_signing_request_file,
            #[cfg(feature = "multisig")]
            multisig::import_signing_request_file,
            #[cfg(feature = "multisig")]
            multisig::export_signature_response_file,
            // Daemon lifecycle
            node_commands::daemon_status,
            node_commands::get_daemon_settings,
            node_commands::set_daemon_settings,
            node_commands::replace_found_node,
            node_commands::start_node,
            node_commands::force_stop_node,
            node_commands::stop_waiting_for_node,
            tor_status::get_tor_status,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Before engine teardown. The page's timer dies with the webview.
                clipboard::clear_on_window_destroy(window.app_handle());

                let app_state: tauri::State<'_, state::AppState> = window.state();
                tauri::async_runtime::block_on(async {
                    let mut eng = app_state.engine.lock().await;
                    let _ = eng.close().await;
                });

                // The node is asked to stop and not waited for: it stops
                // cleanly on its own, and this process's exit closes its
                // owning connection in any case. Nothing here kills it.
                let dm: tauri::State<'_, Arc<daemon_manager::DaemonManager>> = window.state();
                let dm = dm.inner().clone();
                tauri::async_runtime::block_on(async { dm.shutdown().await });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Shekyl Wallet");
}
