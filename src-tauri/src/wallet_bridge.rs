// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Direct FFI bridge to wallet2 via shekyl-wallet-rpc.
//!
//! Combines the C++ wallet2 FFI handle with a Rust scanner backed by
//! `shekyl-scanner`. On `open_wallet`, a background sync loop starts
//! that scans blocks from the daemon, populates `WalletState` with outputs,
//! and detects spends. On `close_wallet` (or window destroy), the sync
//! loop is cancelled and secrets are wiped.
//!
//! The `transfer` flow uses the native-sign path:
//! C++ prepare → Rust sign → C++ finalize, with rollback on failure.

use std::sync::Arc;

use serde::Deserialize;
use shekyl_scanner::WalletState;
use shekyl_wallet_rpc::{ProgressEvent, Wallet2};
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use zeroize::Zeroize;

/// Shared wallet state including both the C++ FFI handle and the Rust scanner.
pub struct WalletBridge {
    pub wallet: Option<Wallet2>,
    /// Scanner state shared with the background sync loop.
    pub scanner_state: Arc<TokioMutex<WalletState>>,
    sync_cancel: Option<CancellationToken>,
}

impl WalletBridge {
    fn new() -> Self {
        WalletBridge {
            wallet: None,
            scanner_state: Arc::new(TokioMutex::new(WalletState::new())),
            sync_cancel: None,
        }
    }
}

/// Shared wallet handle, guarded by a std mutex for synchronous access
/// to the C++ wallet2 FFI. The scanner state is behind a tokio Mutex
/// for async access from both the sync loop and Tauri commands.
pub type WalletHandle = std::sync::Mutex<WalletBridge>;

pub fn new_handle() -> WalletHandle {
    std::sync::Mutex::new(WalletBridge::new())
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

/// Get a clone of the scanner state Arc for async operations.
fn scanner_state(handle: &WalletHandle) -> Result<Arc<TokioMutex<WalletState>>, String> {
    let guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    Ok(guard.scanner_state.clone())
}

fn wallet_err(e: shekyl_wallet_rpc::WalletError) -> String {
    format!("Wallet error: {}", e.message)
}

/// Initialize the wallet2 instance with daemon connection.
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
        .init(
            daemon_address,
            "",   // username: no HTTP digest auth
            "",   // password: no HTTP digest auth
            true, // trusted_daemon
        )
        .map_err(wallet_err)?;
    wallet.set_wallet_dir(wallet_dir);
    guard.wallet = Some(wallet);
    Ok(())
}

/// Check if the wallet instance is initialized.
pub fn is_initialized(handle: &WalletHandle) -> bool {
    handle.lock().map(|g| g.wallet.is_some()).unwrap_or(false)
}

/// Shut down the wallet2 instance and stop the sync loop.
pub fn shutdown(handle: &WalletHandle) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    if let Some(cancel) = guard.sync_cancel.take() {
        cancel.cancel();
    }

    if let Some(wallet) = guard.wallet.as_ref() {
        let _ = wallet.stop();
    }
    guard.wallet = None;

    // Wipe scanner state — replaces the Arc, old WalletState triggers Drop/zeroize
    guard.scanner_state = Arc::new(TokioMutex::new(WalletState::new()));

    Ok(())
}

/// Set up a progress event bridge from wallet2 C++ callbacks to Tauri events.
pub fn setup_progress_bridge(handle: &WalletHandle, app: tauri::AppHandle) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    let wallet = guard.wallet.as_mut().ok_or("Wallet not initialized")?;

    let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();
    wallet.set_progress_sender(tx);

    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let _ = tauri::Emitter::emit(&app, "wallet-progress", &event);
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

/// Open a wallet and start the background scanner sync loop.
///
/// The sync loop runs in a background tokio task, polling the daemon
/// for new blocks and feeding them through the Rust KEM scanner.
pub fn open_wallet(
    handle: &WalletHandle,
    filename: &str,
    password: &str,
    daemon_url: &str,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    let wallet = guard.wallet.as_ref().ok_or("Wallet not initialized")?;
    wallet.open_wallet(filename, password).map_err(wallet_err)?;

    // Extract scanner keys from C++ wallet and start the sync loop
    match wallet.get_scanner_keys() {
        Ok(keys_json) => match start_sync_loop(&mut guard, &keys_json, daemon_url, app) {
            Ok(()) => info!("sync loop started for {filename}"),
            Err(e) => warn!(
                error = %e,
                "failed to start sync loop; wallet opened but scanner inactive"
            ),
        },
        Err(e) => {
            warn!(
                error = %e,
                "failed to get scanner keys; wallet opened but scanner inactive"
            );
        }
    }

    Ok(())
}

/// Close the wallet and stop the sync loop.
pub fn close_wallet(handle: &WalletHandle) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    if let Some(cancel) = guard.sync_cancel.take() {
        cancel.cancel();
        info!("sync loop cancellation requested");
    }

    let wallet = guard.wallet.as_ref().ok_or("Wallet not initialized")?;
    wallet.close_wallet(true).map_err(wallet_err)?;

    // Replace scanner state with a fresh one (old WalletState wipes secrets via Drop)
    guard.scanner_state = Arc::new(TokioMutex::new(WalletState::new()));

    Ok(())
}

// ─── Sync loop management ────────────────────────────────────────────────────

fn start_sync_loop(
    bridge: &mut WalletBridge,
    keys_json: &serde_json::Value,
    daemon_url: &str,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let spend_secret_hex = keys_json["spend_secret"]
        .as_str()
        .ok_or("missing spend_secret")?;
    let view_secret_hex = keys_json["view_secret"]
        .as_str()
        .ok_or("missing view_secret")?;
    let spend_public_hex = keys_json["spend_public"]
        .as_str()
        .ok_or("missing spend_public")?;
    let view_public_hex = keys_json["view_public"]
        .as_str()
        .ok_or("missing view_public")?;
    let x25519_sk_hex = keys_json["x25519_sk"].as_str().ok_or("missing x25519_sk")?;
    let ml_kem_dk_hex = keys_json["ml_kem_dk"].as_str().ok_or("missing ml_kem_dk")?;

    let mut spend_secret = decode_hex_32(spend_secret_hex)?;
    let mut view_secret_array = decode_hex_32(view_secret_hex)?;
    let spend_public_bytes = decode_hex_32(spend_public_hex)?;
    // Decode to validate the hex; the actual view public point is derived
    // from view_scalar * G inside the scanner, so the raw bytes aren't needed.
    let _view_public_bytes = decode_hex_32(view_public_hex)?;
    let mut x25519_sk = decode_hex_32(x25519_sk_hex)?;
    let ml_kem_dk_bytes =
        hex::decode(ml_kem_dk_hex).map_err(|e| format!("invalid ml_kem_dk hex: {e}"))?;

    use curve25519_dalek::edwards::CompressedEdwardsY;
    use curve25519_dalek::scalar::Scalar;

    let spend_point = CompressedEdwardsY::from_slice(&spend_public_bytes)
        .map_err(|_| "invalid spend public key length")?
        .decompress()
        .ok_or("invalid spend public key (decompression failed)")?;

    let view_scalar = Option::from(Scalar::from_canonical_bytes(view_secret_array))
        .ok_or("view secret is not a canonical scalar")?;

    let view_pair = shekyl_scanner::ViewPair::new(
        spend_point,
        zeroize::Zeroizing::new(view_scalar),
        zeroize::Zeroizing::new(x25519_sk),
        zeroize::Zeroizing::new(ml_kem_dk_bytes),
    )
    .map_err(|e| format!("ViewPair error: {e}"))?;

    let scanner = shekyl_scanner::Scanner::new(view_pair, zeroize::Zeroizing::new(spend_secret));

    spend_secret.zeroize();
    view_secret_array.zeroize();
    x25519_sk.zeroize();

    let cancel = CancellationToken::new();
    bridge.sync_cancel = Some(cancel.clone());

    let scanner = Arc::new(TokioMutex::new(scanner));
    let state = bridge.scanner_state.clone();

    let daemon_url_owned = daemon_url.to_string();
    let app_clone = app.clone();

    tokio::spawn(async move {
        let rpc = match shekyl_simple_request_rpc::SimpleRequestRpc::new(format!(
            "http://{daemon_url_owned}"
        ))
        .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "failed to connect to daemon for sync");
                return;
            }
        };

        let result = shekyl_scanner::sync::run_sync_loop(
            rpc,
            scanner,
            state.clone(),
            cancel,
            std::time::Duration::from_secs(5),
            false,
            |progress| {
                let _ = tauri::Emitter::emit(
                    &app_clone,
                    "scanner-progress",
                    &serde_json::json!({
                        "height": progress.height,
                        "daemon_height": progress.daemon_height,
                        "outputs_found": progress.outputs_found,
                        "spends_detected": progress.spends_detected,
                    }),
                );
            },
            |_state| {
                // No on-disk persistence of scanner state yet.
                // On restart the scanner re-scans from the wallet's
                // last-known height. Periodic persistence will be
                // added once WalletState gains serde support.
                tracing::trace!("on_flush: skipped (no persistence yet)");
            },
        )
        .await;

        if let Err(e) = result {
            error!(error = %e, "sync loop exited with error");
        }
    });

    Ok(())
}

fn decode_hex_32(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

// ─── Wallet import ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub struct GetAddressResponse {
    pub address: String,
    #[serde(default)]
    pub addresses: Vec<AddressInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn get_version() -> u32 {
    Wallet2::get_version()
}

// ─── Transfers ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TransferResponse {
    #[serde(default)]
    pub tx_hash: String,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub amount: u64,
    #[serde(default)]
    pub key_images: Vec<String>,
}

/// Execute a transfer via the native-sign path:
/// C++ prepare → Rust sign → C++ finalize.
///
/// No optimistic spent-marking is performed on the scanner side.
/// The scanner's sync loop is the sole authority for marking outputs
/// as spent — it does so only when key images appear on-chain.
/// If finalize fails, outputs remain spendable without rollback.
pub fn transfer(
    handle: &WalletHandle,
    address: &str,
    amount: u64,
) -> Result<TransferResponse, String> {
    with_wallet(handle, |wallet| {
        let dest_json = serde_json::json!([{"amount": amount, "address": address}]).to_string();
        let val = wallet
            .transfer_native(&dest_json, 0, 0)
            .map_err(wallet_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[allow(dead_code)]
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
pub async fn get_scanner_balance(
    handle: &WalletHandle,
) -> Result<shekyl_scanner::BalanceSummary, String> {
    let state_arc = scanner_state(handle)?;
    let state = state_arc.lock().await;
    let height = state.height();
    Ok(state.balance(height))
}

/// Get staked outputs from the Rust scanner state.
pub async fn get_scanner_staked_outputs(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let state_arc = scanner_state(handle)?;
    let state = state_arc.lock().await;
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
}

/// Get claimable staked outputs from the Rust scanner state.
pub async fn get_scanner_claimable_stakes(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let state_arc = scanner_state(handle)?;
    let state = state_arc.lock().await;
    let height = state.height();
    let claimable: Vec<serde_json::Value> = state
        .claimable_outputs(height)
        .iter()
        .map(|td| {
            let accrual_cap = std::cmp::min(height, td.stake_lock_until);
            let watermark = if td.last_claimed_height > 0 {
                td.last_claimed_height
            } else {
                td.block_height
            };
            serde_json::json!({
                "amount": td.amount(),
                "tier": td.stake_tier,
                "lock_until": td.stake_lock_until,
                "from_height": watermark,
                "to_height": accrual_cap,
                "accrual_frozen": height >= td.stake_lock_until,
                "global_output_index": td.global_output_index,
            })
        })
        .collect();
    Ok(serde_json::json!({ "claimable_stakes": claimable }))
}

/// Get unstakeable (matured) outputs from the Rust scanner state.
pub async fn get_scanner_unstakeable_outputs(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let state_arc = scanner_state(handle)?;
    let state = state_arc.lock().await;
    let height = state.height();
    let unstakeable: Vec<serde_json::Value> = state
        .unstakeable_outputs(height)
        .iter()
        .map(|td| {
            serde_json::json!({
                "amount": td.amount(),
                "tier": td.stake_tier,
                "lock_until": td.stake_lock_until,
                "has_unclaimed_backlog": td.has_claimable_rewards(height),
                "global_output_index": td.global_output_index,
            })
        })
        .collect();
    Ok(serde_json::json!({ "unstakeable_outputs": unstakeable }))
}

/// Freeze an output by key image via the scanner state.
pub async fn scanner_freeze(handle: &WalletHandle, key_image_hex: &str) -> Result<bool, String> {
    let ki = parse_key_image(key_image_hex)?;
    let state_arc = scanner_state(handle)?;
    let mut state = state_arc.lock().await;
    Ok(state.freeze_by_key_image(&ki))
}

/// Thaw a frozen output by key image via the scanner state.
pub async fn scanner_thaw(handle: &WalletHandle, key_image_hex: &str) -> Result<bool, String> {
    let ki = parse_key_image(key_image_hex)?;
    let state_arc = scanner_state(handle)?;
    let mut state = state_arc.lock().await;
    Ok(state.thaw_by_key_image(&ki))
}

/// Get the scanner's synced height.
pub async fn get_scanner_height(handle: &WalletHandle) -> Result<u64, String> {
    let state_arc = scanner_state(handle)?;
    let state = state_arc.lock().await;
    Ok(state.height())
}

fn parse_key_image(hex_str: &str) -> Result<[u8; 32], String> {
    if hex_str.len() != 64 {
        return Err(format!(
            "key_image must be 64 hex chars, got {}",
            hex_str.len()
        ));
    }
    decode_hex_32(hex_str)
}

// ─── PQC Multisig signing ────────────────────────────────────────────────────

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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_without_wallet_returns_error_and_preserves_state() {
        let handle = new_handle();
        let result = transfer(&handle, "shekyl1_dummy_address", 1_000_000_000);
        assert!(
            result.is_err(),
            "transfer should fail without initialized wallet"
        );
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("not initialized"),
            "error should indicate wallet not initialized, got: {err_msg}"
        );

        let guard = handle.lock().unwrap();
        assert!(
            guard.wallet.is_none(),
            "wallet should still be None after failed transfer"
        );
        assert!(
            guard.sync_cancel.is_none(),
            "no sync loop should have been created"
        );
    }

    #[test]
    fn scanner_state_survives_transfer_failure() {
        let handle = new_handle();

        let result = transfer(&handle, "shekyl1_dummy_address", 1_000_000_000);
        assert!(result.is_err());

        let guard = handle.lock().unwrap();
        let state_arc = guard.scanner_state.clone();
        drop(guard);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let state = rt.block_on(state_arc.lock());
        assert_eq!(
            state.height(),
            0,
            "scanner state height must be preserved (0) after transfer error"
        );
        assert_eq!(
            state.transfers().len(),
            0,
            "scanner transfers must be empty after transfer error"
        );
    }

    #[test]
    fn shutdown_replaces_scanner_state_with_fresh_instance() {
        let handle = new_handle();

        let original_arc = {
            let guard = handle.lock().unwrap();
            guard.scanner_state.clone()
        };

        shutdown(&handle).unwrap();

        let new_arc = {
            let guard = handle.lock().unwrap();
            guard.scanner_state.clone()
        };

        assert!(
            !Arc::ptr_eq(&original_arc, &new_arc),
            "shutdown must replace scanner_state with a new Arc (old state dropped/zeroized)"
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let state = rt.block_on(new_arc.lock());
        assert_eq!(
            state.height(),
            0,
            "fresh scanner state should have height 0"
        );
    }

    #[test]
    fn close_without_open_returns_error() {
        let handle = new_handle();
        let result = close_wallet(&handle);
        assert!(result.is_err(), "close_wallet without init should error");
    }

    #[test]
    fn multiple_transfers_all_fail_without_corrupting_bridge() {
        let handle = new_handle();
        for i in 0..10 {
            let result = transfer(&handle, "shekyl1_dummy_address", (i + 1) * 1_000_000);
            assert!(result.is_err());
        }

        let guard = handle.lock().unwrap();
        assert!(guard.wallet.is_none());
        assert!(guard.sync_cancel.is_none());
    }

    // NOTE: A full rollback-on-finalize-failure integration test requires a
    // running C++ wallet2 instance with a mock daemon that rejects
    // send_raw_transaction. The prepare → sign → finalize → failure → unmark_spent
    // sequence is handled entirely within C++ wallet2::transfer_native; the Rust
    // bridge does NOT perform optimistic spent-marking (see transfer() doc comment).
    // The unmark_spent logic itself is exercised by Gate 5a tests in
    // shekyl-scanner/src/wallet_state.rs.
}
