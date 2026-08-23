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

mod commands;
mod daemon_manager;
mod daemon_rpc;
mod drain_balance;
mod engine_session;
mod gui_config;
mod shard_visual;
mod staking_view;
mod state;
mod transfer_history;
mod validate;
mod wallet_name;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::new())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let dm = daemon_manager::DaemonManager::new(config_dir);
            app.manage(dm.clone());

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                dm.start(&handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Daemon / chain
            commands::get_wallet_status,
            commands::get_chain_health,
            commands::get_tier_yields,
            commands::set_daemon_connection,
            commands::daemon_connection_disclosures,
            commands::get_pqc_status,
            commands::get_security_status,
            commands::get_curve_tree_info,
            // Mining
            commands::get_mining_status,
            commands::start_mining_cmd,
            commands::stop_mining_cmd,
            // Wallet startup
            commands::check_wallet_files,
            commands::init_wallet_rpc,
            commands::shutdown_wallet_rpc,
            commands::set_wallet_dir,
            commands::reset_wallet_dir,
            commands::get_wallet_dir,
            // Wallet lifecycle
            commands::create_wallet,
            commands::open_wallet,
            commands::close_wallet,
            commands::import_wallet_from_seed,
            commands::import_wallet_from_keys,
            commands::get_seed,
            commands::refresh_wallet,
            commands::get_staker_status,
            commands::activate_staker,
            // Wallet data
            commands::get_balance,
            commands::get_drain_balance,
            commands::get_staking_view,
            commands::get_address,
            commands::transfer,
            commands::estimate_fee,
            commands::get_transactions,
            // Shard identity preview (pre-archival beta)
            shard_visual::list_shard_preview_fixtures,
            shard_visual::render_shard_preview,
            // Shards page (ShardSource-backed; cutover-stable)
            shard_visual::list_shards,
            shard_visual::get_shard_render,
            // PQC Multisig
            commands::create_multisig_group,
            commands::get_multisig_info,
            commands::sign_multisig_partial,
            commands::export_group_descriptor,
            commands::import_group_descriptor,
            commands::export_signing_request_file,
            commands::import_signing_request_file,
            commands::export_signature_response_file,
            // Scanner
            commands::get_scanner_balance,
            commands::get_scanner_height,
            commands::scanner_freeze,
            commands::scanner_thaw,
            // Daemon lifecycle
            commands::daemon_status,
            commands::restart_daemon,
            commands::get_daemon_settings,
            commands::set_daemon_settings,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let app_state: tauri::State<'_, state::AppState> = window.state();
                tauri::async_runtime::block_on(async {
                    let mut eng = app_state.engine.lock().await;
                    let _ = eng.close().await;
                });

                let dm: tauri::State<'_, Arc<daemon_manager::DaemonManager>> = window.state();
                let dm = dm.inner().clone();
                tauri::async_runtime::block_on(async { dm.shutdown().await });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Shekyl Wallet");
}
