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

//! Direct FFI bridge to wallet2 via shekyl-wallet-rpc.
//!
//! Replaces the HTTP JSON-RPC client (`wallet_rpc.rs`) and process manager
//! (`wallet_process.rs`) with direct in-process calls to the C++ wallet2
//! library through the Rust FFI wrapper.

use serde::Deserialize;
use shekyl_wallet_rpc::{ProgressEvent, ScannerState, Wallet2};
use std::sync::Mutex;
use tauri::Emitter;

/// Shared wallet state including both the C++ FFI handle and the Rust scanner.
pub struct WalletBridge {
    pub wallet: Option<Wallet2>,
    pub scanner: ScannerState,
}

impl WalletBridge {
    fn new() -> Self {
        WalletBridge {
            wallet: None,
            scanner: ScannerState::new(),
        }
    }
}

/// Shared wallet handle, guarded by a mutex for thread safety.
pub type WalletHandle = Mutex<WalletBridge>;

pub fn new_handle() -> WalletHandle {
    Mutex::new(WalletBridge::new())
}

fn with_wallet<F, T>(handle: &WalletHandle, f: F) -> Result<T, String>
where
    F: FnOnce(&Wallet2) -> Result<T, String>,
{
    let guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    let wallet = guard.wallet.as_ref().ok_or("Wallet not initialized")?;
    f(wallet)
}

fn with_scanner<F, T>(handle: &WalletHandle, f: F) -> Result<T, String>
where
    F: FnOnce(&ScannerState) -> Result<T, String>,
{
    let guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    f(&guard.scanner)
}

fn wallet_err(e: shekyl_wallet_rpc::WalletError) -> String {
    format!("Wallet error: {}", e.message)
}

/// Initialize the wallet2 instance with daemon connection.
/// Replaces `wallet_process::spawn_wallet_rpc` + `wait_for_ready`.
pub fn init(
    handle: &WalletHandle,
    nettype: u8,
    daemon_address: &str,
    wallet_dir: &str,
) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    if guard.wallet.is_some() {
        return Ok(());
    }

    let wallet = Wallet2::new(nettype).map_err(wallet_err)?;
    wallet
        .init(daemon_address, "", "", true)
        .map_err(wallet_err)?;
    wallet.set_wallet_dir(wallet_dir);
    guard.wallet = Some(wallet);
    Ok(())
}

/// Check if the wallet instance is initialized.
pub fn is_initialized(handle: &WalletHandle) -> bool {
    handle.lock().map(|g| g.wallet.is_some()).unwrap_or(false)
}

/// Shut down the wallet2 instance. Replaces `wallet_process::shutdown`.
pub fn shutdown(handle: &WalletHandle) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    if let Some(wallet) = guard.wallet.as_ref() {
        let _ = wallet.stop();
    }
    guard.wallet = None;
    Ok(())
}

/// Set up a progress event bridge from wallet2 C++ callbacks to Tauri events.
/// Spawns a background thread that reads from a channel and emits
/// `wallet-progress` events to the frontend.
pub fn setup_progress_bridge(handle: &WalletHandle, app: tauri::AppHandle) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    let wallet = guard.wallet.as_mut().ok_or("Wallet not initialized")?;

    let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();
    wallet.set_progress_sender(tx);

    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let _ = app.emit("wallet-progress", &event);
        }
    });
    Ok(())
}

// ─── Wallet lifecycle ────────────────────────────────────────────────────────

pub fn create_wallet(
    handle: &WalletHandle,
    filename: &str,
    password: &str,
    language: &str,
) -> Result<(), String> {
    with_wallet(handle, |w| {
        w.create_wallet(filename, password, language)
            .map_err(wallet_err)
    })
}

pub fn open_wallet(handle: &WalletHandle, filename: &str, password: &str) -> Result<(), String> {
    with_wallet(handle, |w| {
        w.open_wallet(filename, password).map_err(wallet_err)
    })
}

pub fn close_wallet(handle: &WalletHandle) -> Result<(), String> {
    with_wallet(handle, |w| w.close_wallet(true).map_err(wallet_err))
}

// ─── Wallet import ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RestoreWalletResponse {
    pub address: String,
    #[serde(default)]
    pub seed: String,
    #[serde(default)]
    pub info: String,
    #[serde(default)]
    pub was_deprecated: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn restore_deterministic_wallet(
    handle: &WalletHandle,
    filename: &str,
    seed: &str,
    password: &str,
    language: &str,
    restore_height: u64,
    seed_offset: &str,
) -> Result<RestoreWalletResponse, String> {
    with_wallet(handle, |w| {
        let val = w
            .restore_deterministic_wallet(
                filename,
                seed,
                password,
                language,
                restore_height,
                seed_offset,
            )
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize)]
pub struct GenerateFromKeysResponse {
    pub address: String,
    #[serde(default)]
    pub info: String,
}

#[allow(clippy::too_many_arguments)]
pub fn generate_from_keys(
    handle: &WalletHandle,
    filename: &str,
    address: &str,
    spendkey: &str,
    viewkey: &str,
    password: &str,
    language: &str,
    restore_height: u64,
) -> Result<GenerateFromKeysResponse, String> {
    with_wallet(handle, |w| {
        let val = w
            .generate_from_keys(
                filename,
                address,
                spendkey,
                viewkey,
                password,
                language,
                restore_height,
            )
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

// ─── Queries ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetAddressResponse {
    pub address: String,
    #[serde(default)]
    pub addresses: Vec<AddressInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AddressInfo {
    pub address: String,
    #[serde(default)]
    pub label: String,
    pub address_index: u32,
    #[serde(default)]
    pub used: bool,
}

pub fn get_address(
    handle: &WalletHandle,
    account_index: u32,
) -> Result<GetAddressResponse, String> {
    with_wallet(handle, |w| {
        let val = w.get_address(account_index).map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize)]
pub struct GetBalanceResponse {
    pub balance: u64,
    pub unlocked_balance: u64,
    #[serde(default)]
    pub blocks_to_unlock: u64,
}

pub fn get_balance(
    handle: &WalletHandle,
    account_index: u32,
) -> Result<GetBalanceResponse, String> {
    with_wallet(handle, |w| {
        let val = w.get_balance(account_index).map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn query_key(handle: &WalletHandle, key_type: &str) -> Result<String, String> {
    with_wallet(handle, |w| {
        let val = w.query_key(key_type).map_err(wallet_err)?;
        val["key"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| "Missing 'key' field in response".into())
    })
}

pub fn get_version() -> u32 {
    Wallet2::get_version()
}

// ─── Transfers ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TransferResponse {
    #[serde(default)]
    pub tx_hash: String,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub amount: u64,
}

pub fn transfer(
    handle: &WalletHandle,
    address: &str,
    amount: u64,
) -> Result<TransferResponse, String> {
    with_wallet(handle, |w| {
        let dest_json = serde_json::json!([{"amount": amount, "address": address}]).to_string();
        let val = w.transfer(&dest_json, 0, 0, 0).map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize)]
pub struct TransferEntry {
    #[serde(default)]
    pub txid: String,
    #[serde(default)]
    pub amount: u64,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub height: u64,
    #[serde(default)]
    pub timestamp: u64,
    #[serde(rename = "type", default)]
    pub transfer_type: String,
    #[serde(default)]
    pub confirmations: u64,
    #[serde(default)]
    pub pqc_protected: bool,
}

#[derive(Debug, Deserialize)]
pub struct GetTransfersResponse {
    #[serde(default)]
    pub r#in: Vec<TransferEntry>,
    #[serde(default)]
    pub out: Vec<TransferEntry>,
    #[serde(default)]
    pub pending: Vec<TransferEntry>,
    #[serde(default)]
    pub pool: Vec<TransferEntry>,
}

pub fn get_transfers(
    handle: &WalletHandle,
    r#in: bool,
    out: bool,
    pending: bool,
    pool: bool,
) -> Result<GetTransfersResponse, String> {
    with_wallet(handle, |w| {
        let val = w
            .get_transfers(r#in, out, pending, false, pool, 0)
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn stop_wallet(handle: &WalletHandle) -> Result<(), String> {
    with_wallet(handle, |w| w.stop().map_err(wallet_err))
}

// ─── Staking ─────────────────────────────────────────────────────────────────

pub fn stake(handle: &WalletHandle, tier: u8, amount: u64) -> Result<TransferResponse, String> {
    with_wallet(handle, |w| {
        let params = serde_json::json!({ "tier": tier, "amount": amount });
        let val = w
            .json_rpc_call("stake", &params.to_string())
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn claim_rewards(handle: &WalletHandle) -> Result<TransferResponse, String> {
    with_wallet(handle, |w| {
        let val = w.json_rpc_call("claim_rewards", "{}").map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize)]
pub struct StakedOutput {
    #[serde(default)]
    pub amount: u64,
    #[serde(default)]
    pub tier: u8,
    #[serde(default)]
    pub lock_height: u64,
    #[serde(default)]
    pub unlock_height: u64,
    #[serde(default)]
    pub claimable: bool,
}

#[derive(Debug, Deserialize)]
pub struct GetStakedOutputsResponse {
    #[serde(default)]
    pub staked_outputs: Vec<StakedOutput>,
    #[serde(default)]
    pub total_staked: u64,
}

pub fn get_staked_outputs(handle: &WalletHandle) -> Result<GetStakedOutputsResponse, String> {
    with_wallet(handle, |w| {
        let val = w
            .json_rpc_call("get_staked_outputs", "{}")
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

// ─── PQC Multisig ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateMultisigGroupResponse {
    pub group_id: String,
    pub n_total: u8,
    pub m_required: u8,
}

pub fn create_pqc_multisig_group(
    handle: &WalletHandle,
    n_total: u8,
    m_required: u8,
    participant_keys: Vec<String>,
) -> Result<CreateMultisigGroupResponse, String> {
    with_wallet(handle, |w| {
        let params = serde_json::json!({
            "n_total": n_total,
            "m_required": m_required,
            "participant_keys": participant_keys,
        });
        let val = w
            .json_rpc_call("create_pqc_multisig_group", &params.to_string())
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct PqcMultisigInfo {
    pub is_multisig: bool,
    #[serde(default)]
    pub n_total: u8,
    #[serde(default)]
    pub m_required: u8,
    #[serde(default)]
    pub group_id: String,
}

pub fn get_pqc_multisig_info(handle: &WalletHandle) -> Result<PqcMultisigInfo, String> {
    with_wallet(handle, |w| {
        let val = w
            .json_rpc_call("get_pqc_multisig_info", "{}")
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

// ─── Scanner-backed queries ──────────────────────────────────────────────────

/// Get balance from the Rust scanner state.
///
/// Returns the scanner's view of the balance at its current synced height.
/// If the scanner hasn't synced yet, the balance will be zero.
pub fn get_scanner_balance(
    handle: &WalletHandle,
) -> Result<shekyl_scanner::BalanceSummary, String> {
    with_scanner(handle, |scanner| {
        let state = scanner
            .state
            .lock()
            .map_err(|e| format!("Scanner lock: {e}"))?;
        let height = state.height();
        Ok(state.balance(height))
    })
}

/// Get staked outputs from the Rust scanner state.
pub fn get_scanner_staked_outputs(handle: &WalletHandle) -> Result<serde_json::Value, String> {
    with_scanner(handle, |scanner| {
        let state = scanner
            .state
            .lock()
            .map_err(|e| format!("Scanner lock: {e}"))?;
        let height = state.height();
        let staked: Vec<serde_json::Value> = state
            .staked_outputs()
            .iter()
            .map(|td| {
                serde_json::json!({
                    "amount": td.amount(),
                    "tier": td.stake_tier,
                    "lock_height": td.block_height,
                    "unlock_height": td.stake_lock_until,
                    "claimable": td.is_matured_stake(height),
                })
            })
            .collect();
        Ok(serde_json::json!({ "staked_outputs": staked }))
    })
}

/// Get the scanner's synced height.
pub fn get_scanner_height(handle: &WalletHandle) -> Result<u64, String> {
    with_scanner(handle, |scanner| {
        let state = scanner
            .state
            .lock()
            .map_err(|e| format!("Scanner lock: {e}"))?;
        Ok(state.height())
    })
}

#[derive(Debug, Deserialize)]
pub struct SignMultisigResponse {
    pub signature_response: String,
}

pub fn sign_multisig_partial(
    handle: &WalletHandle,
    signing_request: &str,
) -> Result<SignMultisigResponse, String> {
    with_wallet(handle, |w| {
        let params = serde_json::json!({ "signing_request": signing_request });
        let val = w
            .json_rpc_call("sign_multisig_partial", &params.to_string())
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}
