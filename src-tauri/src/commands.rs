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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct WalletInfo {
    pub name: String,
    pub address: String,
    pub seed_language: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct Balance {
    pub total: u64,
    pub unlocked: u64,
    pub staked: u64,
}

#[derive(Debug, Serialize)]
pub struct TxInfo {
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    pub height: u64,
    pub timestamp: u64,
    pub direction: String,
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
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
pub async fn transfer(_address: String, _amount: u64) -> Result<TxInfo, String> {
    Err("Wallet not connected — stub implementation".into())
}

#[tauri::command]
pub async fn get_transactions(_offset: u32, _limit: u32) -> Result<Vec<TxInfo>, String> {
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wallet_status_defaults_to_disconnected() {
        let status = get_wallet_status().await.unwrap();
        assert!(!status.connected);
        assert!(!status.wallet_open);
        assert!(status.wallet_name.is_none());
        assert!(status.daemon_address.is_none());
        assert_eq!(status.network, "mainnet");
        assert!(!status.synced);
        assert_eq!(status.sync_height, 0);
        assert_eq!(status.daemon_height, 0);
    }

    #[tokio::test]
    async fn create_wallet_returns_provided_name() {
        let info = create_wallet("Test".into(), "pass".into()).await.unwrap();
        assert_eq!(info.name, "Test");
        assert_eq!(info.network, "mainnet");
        assert_eq!(info.seed_language, "English");
        assert!(!info.address.is_empty());
    }

    #[tokio::test]
    async fn open_wallet_returns_mock_wallet() {
        let info = open_wallet("/tmp/w".into(), "pass".into()).await.unwrap();
        assert_eq!(info.name, "Mock Wallet");
        assert!(!info.address.is_empty());
    }

    #[tokio::test]
    async fn close_wallet_returns_true() {
        assert!(close_wallet().await.unwrap());
    }

    #[tokio::test]
    async fn balance_defaults_to_zero() {
        let b = get_balance().await.unwrap();
        assert_eq!(b.total, 0);
        assert_eq!(b.unlocked, 0);
        assert_eq!(b.staked, 0);
    }

    #[tokio::test]
    async fn get_address_encodes_account_and_index() {
        let addr = get_address(1, 2).await.unwrap();
        assert!(addr.contains("account1"));
        assert!(addr.contains("subaddr2"));
    }

    #[tokio::test]
    async fn transfer_returns_error_when_disconnected() {
        let err = transfer("SKL1abc".into(), 1_000_000).await.unwrap_err();
        assert!(err.contains("not connected"));
    }

    #[tokio::test]
    async fn get_transactions_returns_empty_vec() {
        let txs = get_transactions(0, 10).await.unwrap();
        assert!(txs.is_empty());
    }

    #[tokio::test]
    async fn staking_info_defaults_to_inactive() {
        let info = get_staking_info().await.unwrap();
        assert!(!info.active);
        assert!(info.tier.is_none());
        assert_eq!(info.staked_amount, 0);
        assert_eq!(info.rewards_pending, 0);
        assert!((info.stake_ratio - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn stake_returns_error_when_disconnected() {
        let err = stake(1, 5_000_000).await.unwrap_err();
        assert!(err.contains("not connected"));
    }
}
