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

use tauri::Manager;

mod commands;
mod daemon_rpc;
mod state;
mod wallet_process;
mod wallet_rpc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Daemon / chain
            commands::get_wallet_status,
            commands::get_chain_health,
            commands::get_tier_yields,
            commands::set_daemon_connection,
            commands::get_pqc_status,
            // Mining
            commands::get_mining_status,
            commands::start_mining_cmd,
            commands::stop_mining_cmd,
            // Wallet startup
            commands::check_wallet_files,
            commands::init_wallet_rpc,
            commands::shutdown_wallet_rpc,
            // Wallet lifecycle
            commands::create_wallet,
            commands::open_wallet,
            commands::close_wallet,
            commands::import_wallet_from_seed,
            commands::import_wallet_from_keys,
            commands::get_seed,
            // Wallet data
            commands::get_balance,
            commands::get_address,
            commands::transfer,
            commands::get_transactions,
            commands::get_staking_info,
            commands::stake,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let app_state: tauri::State<'_, state::AppState> = window.state();
                let mut proc = app_state
                    .wallet_process
                    .lock()
                    .unwrap_or_else(|e: std::sync::PoisonError<_>| e.into_inner());
                if let Some(ref mut child) = *proc {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                *proc = None;
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Shekyl Wallet");
}
