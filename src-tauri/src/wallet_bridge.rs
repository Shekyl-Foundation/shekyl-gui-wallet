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
use shekyl_wallet_rpc::Wallet2;
use std::sync::Mutex;

/// Shared wallet handle, guarded by a mutex for thread safety.
/// `None` means no wallet instance has been initialized yet.
pub type WalletHandle = Mutex<Option<Wallet2>>;

pub fn new_handle() -> WalletHandle {
    Mutex::new(None)
}

fn with_wallet<F, T>(handle: &WalletHandle, f: F) -> Result<T, String>
where
    F: FnOnce(&Wallet2) -> Result<T, String>,
{
    let guard = handle.lock().map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    let wallet = guard.as_ref().ok_or("Wallet not initialized")?;
    f(wallet)
}

fn wallet_err(e: shekyl_wallet_rpc::WalletError) -> String {
    format!("Wallet error: {}", e.message)
}

/// Initialize the wallet2 instance with daemon connection.
/// Replaces `wallet_process::spawn_wallet_rpc` + `wait_for_ready`.
pub fn init(handle: &WalletHandle, nettype: u8, daemon_address: &str, wallet_dir: &str) -> Result<(), String> {
    let mut guard = handle.lock().map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    if guard.is_some() {
        return Ok(());
    }

    let wallet = Wallet2::new(nettype).map_err(wallet_err)?;
    wallet.init(daemon_address, "", "", true).map_err(wallet_err)?;
    wallet.set_wallet_dir(wallet_dir);
    *guard = Some(wallet);
    Ok(())
}

/// Check if the wallet instance is initialized.
pub fn is_initialized(handle: &WalletHandle) -> bool {
    handle.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Shut down the wallet2 instance. Replaces `wallet_process::shutdown`.
pub fn shutdown(handle: &WalletHandle) -> Result<(), String> {
    let mut guard = handle.lock().map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    if let Some(wallet) = guard.as_ref() {
        let _ = wallet.stop();
    }
    *guard = None;
    Ok(())
}

// ─── Wallet lifecycle ────────────────────────────────────────────────────────

pub fn create_wallet(handle: &WalletHandle, filename: &str, password: &str, language: &str) -> Result<(), String> {
    with_wallet(handle, |w| w.create_wallet(filename, password, language).map_err(wallet_err))
}

pub fn open_wallet(handle: &WalletHandle, filename: &str, password: &str) -> Result<(), String> {
    with_wallet(handle, |w| w.open_wallet(filename, password).map_err(wallet_err))
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
            .restore_deterministic_wallet(filename, seed, password, language, restore_height, seed_offset)
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
            .generate_from_keys(filename, address, spendkey, viewkey, password, language, restore_height)
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

pub fn get_address(handle: &WalletHandle, account_index: u32) -> Result<GetAddressResponse, String> {
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

pub fn get_balance(handle: &WalletHandle, account_index: u32) -> Result<GetBalanceResponse, String> {
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

pub fn transfer(handle: &WalletHandle, address: &str, amount: u64) -> Result<TransferResponse, String> {
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
        let val = w.get_transfers(r#in, out, pending, false, pool, 0).map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn stop_wallet(handle: &WalletHandle) -> Result<(), String> {
    with_wallet(handle, |w| w.stop().map_err(wallet_err))
}
