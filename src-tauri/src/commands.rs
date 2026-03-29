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

//! Tauri command stubs for the Shekyl wallet.
//!
//! These return mock data so the UI can be developed end-to-end.
//! Phase 2 will replace them with real calls through the C++ FFI bridge
//! to wallet2_api.h.

use serde::Serialize;

// ─── Data types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct WalletStatus {
    pub connected: bool,
    pub wallet_open: bool,
    pub wallet_name: Option<String>,
    pub daemon_address: Option<String>,
    pub network: String,
    pub synced: bool,
    pub sync_height: u64,
    pub daemon_height: u64,
}

#[derive(Serialize)]
pub struct WalletInfo {
    pub name: String,
    pub address: String,
    pub seed_language: String,
    pub network: String,
}

#[derive(Serialize)]
pub struct Balance {
    pub total: u64,
    pub unlocked: u64,
    pub staked: u64,
}

#[derive(Serialize)]
pub struct TxInfo {
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    pub height: u64,
    pub timestamp: u64,
    pub direction: String,
    pub confirmed: bool,
}

#[derive(Serialize)]
pub struct StakingInfo {
    pub active: bool,
    pub tier: Option<u8>,
    pub staked_amount: u64,
    pub unlock_height: Option<u64>,
    pub rewards_pending: u64,
    pub stake_ratio: f64,
}

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_wallet_status() -> Result<WalletStatus, String> {
    Ok(WalletStatus {
        connected: false,
        wallet_open: false,
        wallet_name: None,
        daemon_address: None,
        network: "mainnet".into(),
        synced: false,
        sync_height: 0,
        daemon_height: 0,
    })
}

#[tauri::command]
pub async fn create_wallet(name: String, _password: String) -> Result<WalletInfo, String> {
    Ok(WalletInfo {
        name,
        address: "SKL1mock...placeholder".into(),
        seed_language: "English".into(),
        network: "mainnet".into(),
    })
}

#[tauri::command]
pub async fn open_wallet(_path: String, _password: String) -> Result<WalletInfo, String> {
    Ok(WalletInfo {
        name: "Mock Wallet".into(),
        address: "SKL1mock...placeholder".into(),
        seed_language: "English".into(),
        network: "mainnet".into(),
    })
}

#[tauri::command]
pub async fn close_wallet() -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub async fn get_balance() -> Result<Balance, String> {
    Ok(Balance {
        total: 0,
        unlocked: 0,
        staked: 0,
    })
}

#[tauri::command]
pub async fn get_address(account: u32, index: u32) -> Result<String, String> {
    Ok(format!(
        "SKL1mock_account{}_subaddr{}...placeholder",
        account, index
    ))
}

#[tauri::command]
pub async fn transfer(
    _address: String,
    _amount: u64,
) -> Result<TxInfo, String> {
    Err("Wallet not connected — stub implementation".into())
}

#[tauri::command]
pub async fn get_transactions(
    _offset: u32,
    _limit: u32,
) -> Result<Vec<TxInfo>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn get_staking_info() -> Result<StakingInfo, String> {
    Ok(StakingInfo {
        active: false,
        tier: None,
        staked_amount: 0,
        unlock_height: None,
        rewards_pending: 0,
        stake_ratio: 0.0,
    })
}

#[tauri::command]
pub async fn stake(_tier: u8, _amount: u64) -> Result<TxInfo, String> {
    Err("Wallet not connected — stub implementation".into())
}
