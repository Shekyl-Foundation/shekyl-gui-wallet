// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Direct FFI bridge to wallet2 via shekyl-engine-rpc.
//!
//! Combines the C++ wallet2 FFI handle with a Rust scanner backed by
//! `shekyl-scanner`. On `open_wallet`, a background sync loop starts
//! that scans blocks from the daemon, populates a
//! `(LedgerBlock, LedgerIndexes)` pair with outputs, and detects spends.
//! On `close_wallet` (or window destroy), the sync loop is cancelled and
//! secrets are wiped.
//!
//! The `transfer` flow uses the native-sign path:
//! C++ prepare → Rust sign → C++ finalize, with rollback on failure.
//!
//! ### Why the in-process loop instead of `shekyl_scanner::sync`
//!
//! `shekyl_scanner::sync::run_sync_loop` was retired in `shekyl-core`
//! commit `252d942d2` (2026-04-26) as part of the
//! `RuntimeWalletState` → `(LedgerBlock, LedgerIndexes)` migration.
//! The forward-looking driver lives at
//! [`shekyl_engine_core::Engine::start_refresh`], which presumes the
//! Rust-native `Engine` owns wallet state. The GUI still bridges
//! through the C++ `wallet2` FFI (`Wallet2`); switching to `Engine`
//! waits for the wallet rewrite (`docs/FOLLOWUPS.md`: "Adopt
//! `Engine::start_refresh` / `RefreshHandle`"). Until then, this
//! module carries the smallest possible local sync loop driving the
//! new `(LedgerBlock, LedgerIndexes)` shape.

use std::sync::Arc;

use serde::Deserialize;
use shekyl_crypto_pq::key_image::KeyImage;
use shekyl_engine_core::DaemonClient;
use shekyl_engine_rpc::{ProgressEvent, Wallet2};
use shekyl_rpc_client::{Rpc, RpcError};
use shekyl_rpc_transport::SimpleRequestRpc;
use shekyl_scanner::{
    LedgerBlock, LedgerBlockExt, LedgerIndexes, LedgerIndexesExt, ScannableBlock,
};
use shekyl_wire::Input;
use tokio::sync::Mutex as TokioMutex;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use zeroize::Zeroize;

/// Shared wallet state including both the C++ FFI handle and the Rust scanner.
pub struct WalletBridge {
    pub wallet: Option<Wallet2>,
    /// Scanner state shared with the background sync loop.
    ///
    /// `LedgerBlock` is the persisted on-chain-derived state;
    /// `LedgerIndexes` is the runtime-only lookup-and-accrual state.
    /// Held under a single lock because every block-ingestion path
    /// mutates both — splitting the lock would invite torn observation
    /// windows.
    pub scanner_state: Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>,
    sync_cancel: Option<CancellationToken>,
}

impl WalletBridge {
    fn new() -> Self {
        WalletBridge {
            wallet: None,
            scanner_state: Arc::new(TokioMutex::new((
                LedgerBlock::empty(),
                LedgerIndexes::empty(),
            ))),
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
fn scanner_state(
    handle: &WalletHandle,
) -> Result<Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>, String> {
    let guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;
    Ok(guard.scanner_state.clone())
}

fn engine_err(e: shekyl_engine_rpc::EngineError) -> String {
    format!("Wallet error: {}", e.message)
}

/// Initialize the wallet2 instance with daemon connection.
///
/// `wallet2_ffi` no longer tracks a base wallet directory on the C++
/// side; callers build full paths in Rust via [`crate::wallet_name`] and
/// pass them to the create/open/restore functions below. See
/// `shekyl-core` `docs/CHANGELOG.md` §"wallet2_ffi no longer carries
/// wallet-directory state".
pub fn init(handle: &WalletHandle, nettype: u8, daemon_address: &str) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    if guard.wallet.is_some() {
        return Ok(());
    }

    let wallet = Wallet2::new(nettype).map_err(engine_err)?;
    wallet
        .init(
            daemon_address,
            "",   // username: no HTTP digest auth
            "",   // password: no HTTP digest auth
            true, // trusted_daemon
        )
        .map_err(engine_err)?;
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

    // Replace scanner state with a fresh pair (old tuple's transfers
    // wipe via TransferDetails' own zeroize discipline as it drops).
    guard.scanner_state = Arc::new(TokioMutex::new((
        LedgerBlock::empty(),
        LedgerIndexes::empty(),
    )));

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
    wallet_path: &str,
    password: &str,
    language: &str,
) -> Result<(), String> {
    with_wallet(handle, |w| {
        w.create_wallet(wallet_path, password, language)
            .map_err(engine_err)
    })
}

/// Open a wallet and start the background scanner sync loop.
///
/// The sync loop runs in a background tokio task, polling the daemon
/// for new blocks and feeding them through the Rust KEM scanner.
pub fn open_wallet(
    handle: &WalletHandle,
    wallet_path: &str,
    password: &str,
    daemon_url: &str,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut guard = handle
        .lock()
        .map_err(|e| format!("Wallet lock poisoned: {e}"))?;

    let wallet = guard.wallet.as_ref().ok_or("Wallet not initialized")?;
    wallet
        .open_wallet(wallet_path, password)
        .map_err(engine_err)?;

    // Extract scanner keys from C++ wallet and start the sync loop
    match wallet.get_scanner_keys() {
        Ok(keys_json) => match start_sync_loop(&mut guard, &keys_json, daemon_url, app) {
            Ok(()) => info!("sync loop started"),
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
    wallet.close_wallet(true).map_err(engine_err)?;

    // Replace scanner state with a fresh pair (old state drops, wiping
    // any TransferDetails-resident secrets via their own zeroize impls).
    guard.scanner_state = Arc::new(TokioMutex::new((
        LedgerBlock::empty(),
        LedgerIndexes::empty(),
    )));

    Ok(())
}

// ─── Sync loop management ────────────────────────────────────────────────────

/// Daemon polling interval when at-tip.
const SYNC_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
/// Maximum retries for transient per-block RPC failures.
const MAX_BLOCK_FETCH_RETRIES: u32 = 5;
/// Initial backoff for block-fetch retries; doubles each failure up to 30s.
const INITIAL_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

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
        let rpc = match SimpleRequestRpc::new(format!("http://{daemon_url_owned}")).await {
            Ok(r) => DaemonClient::new(r),
            Err(e) => {
                error!(error = %e, "failed to connect to daemon for sync");
                return;
            }
        };

        if let Err(e) = run_local_sync_loop(rpc, scanner, state, cancel, app_clone).await {
            error!(error = %e, "sync loop exited with error");
        }
    });

    Ok(())
}

/// Sync-loop error class. Both branches stop the loop; the caller logs.
#[derive(Debug, thiserror::Error)]
enum SyncError {
    #[error("rpc error: {0}")]
    Rpc(#[from] RpcError),
    #[error("scan error: {0}")]
    Scan(#[from] shekyl_scanner::ScanError),
}

/// Local replacement for the retired `shekyl_scanner::sync::run_sync_loop`.
///
/// Drives the new `(LedgerBlock, LedgerIndexes)` shape directly. Mirrors
/// the previous loop's contract:
///
/// 1. Poll `rpc.get_height()` every [`SYNC_POLL_INTERVAL`] when at-tip.
/// 2. Fetch blocks `wallet_height + 1 ..= daemon_height` with bounded
///    retry/backoff via [`fetch_block_with_retry`].
/// 3. Detect reorgs by comparing each block's `previous` hash against
///    the stored hash for the prior height; on mismatch, walk back to
///    the fork point and call [`LedgerIndexes::handle_reorg`].
/// 4. Per accepted block: `Scanner::scan` → owned-output set →
///    `LedgerIndexesExt::process_scanned_outputs` (atomically updates
///    the ledger), then `LedgerIndexes::detect_spends` against the
///    block's miner + non-miner key images.
/// 5. Emit a `scanner-progress` Tauri event after each block.
async fn run_local_sync_loop(
    rpc: DaemonClient,
    scanner: Arc<TokioMutex<shekyl_scanner::Scanner>>,
    state: Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>,
    cancel: CancellationToken,
    app: tauri::AppHandle,
) -> Result<(), SyncError> {
    info!("sync loop started");

    'outer: loop {
        if cancel.is_cancelled() {
            info!("sync loop cancelled");
            break;
        }

        let daemon_height = match rpc.get_height().await {
            Ok(h) => h as u64,
            Err(e) => {
                warn!(error = %e, "failed to get daemon height, retrying after poll interval");
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(SYNC_POLL_INTERVAL) => continue,
                }
            }
        };

        let wallet_height = {
            let guard = state.lock().await;
            guard.0.height()
        };

        if wallet_height >= daemon_height {
            debug!(wallet_height, daemon_height, "wallet is synced, sleeping");
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(SYNC_POLL_INTERVAL) => continue,
            }
        }

        let start_height = wallet_height + 1;

        for h in start_height..=daemon_height {
            if cancel.is_cancelled() {
                info!(height = h, "sync loop cancelled mid-batch");
                break 'outer;
            }

            let scannable = fetch_block_with_retry(&rpc, h, &cancel).await?;

            // --- Reorg detection ---
            // Compare the block's `previous` hash against what we stored
            // for `h - 1`. Mismatch means the chain forked while we were
            // away; walk back to the fork point and roll the ledger.
            if h > 1 {
                let parent_hash = scannable.block.header.previous;
                let expected = {
                    let guard = state.lock().await;
                    guard.0.block_hash_at(h - 1).copied()
                };

                if let Some(stored_hash) = expected {
                    if stored_hash != parent_hash {
                        warn!(
                            height = h,
                            expected = hex::encode(stored_hash),
                            actual_parent = hex::encode(parent_hash),
                            "chain reorg detected, rolling back"
                        );

                        let fork_height = find_fork_point(&rpc, &state, h - 1, &cancel).await?;

                        {
                            let mut guard = state.lock().await;
                            let (ledger, indexes) = &mut *guard;
                            indexes.handle_reorg(ledger, fork_height);
                        }

                        info!(
                            fork_height,
                            "reorg handled, restarting scan from fork point"
                        );
                        continue 'outer;
                    }
                }
            }

            let block_hash = scannable.block.hash();

            let outputs = {
                let mut scanner_guard = scanner.lock().await;
                match scanner_guard.scan(scannable.clone()) {
                    Ok(t) => t,
                    Err(e) => {
                        error!(height = h, error = %e, "scan failed, aborting batch");
                        return Err(SyncError::Scan(e));
                    }
                }
            };

            // Collect every key image consumed by this block (miner + non-miner).
            // Spends are `Input::ToKey` with a raw `[u8; 32]` key image on the
            // shekyl-wire surface (StakeClaim was deleted with the oxide dissolve).
            let mut block_key_images: Vec<KeyImage> = Vec::new();
            for input in &scannable.block.miner_transaction.prefix.inputs {
                if let Input::ToKey { key_image, .. } = input {
                    block_key_images.push(KeyImage::from_canonical_bytes(*key_image));
                }
            }
            for tx in &scannable.transactions {
                for input in &tx.prefix.inputs {
                    if let Input::ToKey { key_image, .. } = input {
                        block_key_images.push(KeyImage::from_canonical_bytes(*key_image));
                    }
                }
            }

            let (outputs_found, spends_detected) = {
                let mut guard = state.lock().await;
                let (ledger, indexes) = &mut *guard;
                let range = indexes.process_scanned_outputs(ledger, h, block_hash, outputs);
                let found = range.len();
                let spent = indexes.detect_spends(ledger, h, &block_key_images);
                (found, spent)
            };

            if outputs_found > 0 || spends_detected > 0 {
                info!(
                    height = h,
                    outputs_found, spends_detected, "block processed with wallet activity"
                );
            }

            let _ = tauri::Emitter::emit(
                &app,
                "scanner-progress",
                &serde_json::json!({
                    "height": h,
                    "daemon_height": daemon_height,
                    "outputs_found": outputs_found,
                    "spends_detected": spends_detected,
                }),
            );
        }
    }

    info!("sync loop stopped");
    Ok(())
}

/// Fetch a block with exponential backoff on transient failures.
async fn fetch_block_with_retry(
    rpc: &DaemonClient,
    height: u64,
    cancel: &CancellationToken,
) -> Result<ScannableBlock, SyncError> {
    let mut delay = INITIAL_RETRY_DELAY;
    for attempt in 0..MAX_BLOCK_FETCH_RETRIES {
        match rpc.fetch_scannable_block(height as usize).await {
            Ok(b) => return Ok(b),
            Err(e) if attempt + 1 < MAX_BLOCK_FETCH_RETRIES => {
                warn!(
                    height,
                    attempt = attempt + 1,
                    max = MAX_BLOCK_FETCH_RETRIES,
                    error = %e,
                    "block fetch failed, retrying after backoff"
                );
                tokio::select! {
                    _ = cancel.cancelled() => return Err(SyncError::Rpc(e)),
                    _ = tokio::time::sleep(delay) => {}
                }
                delay = std::cmp::min(delay * 2, std::time::Duration::from_secs(30));
            }
            Err(e) => {
                error!(
                    height,
                    error = %e,
                    "block fetch failed after {} attempts, aborting",
                    MAX_BLOCK_FETCH_RETRIES,
                );
                return Err(SyncError::Rpc(e));
            }
        }
    }
    unreachable!()
}

/// Walk backwards from `from_height` to find the fork point where the
/// stored block hash matches the daemon's chain.
async fn find_fork_point(
    rpc: &DaemonClient,
    state: &Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>,
    from_height: u64,
    cancel: &CancellationToken,
) -> Result<u64, SyncError> {
    let mut h = from_height;
    loop {
        if h == 0 {
            return Ok(1);
        }
        if cancel.is_cancelled() {
            return Ok(h + 1);
        }

        let stored = {
            let guard = state.lock().await;
            guard.0.block_hash_at(h).copied()
        };

        let Some(stored_hash) = stored else {
            return Ok(h + 1);
        };

        let daemon_block = fetch_block_with_retry(rpc, h, cancel).await?;
        let daemon_hash = daemon_block.block.hash();

        if daemon_hash == stored_hash {
            return Ok(h + 1);
        }

        debug!(height = h, "fork point search: mismatch, going back");
        h -= 1;
    }
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
    wallet_path: &str,
    seed: &str,
    password: &str,
    language: &str,
    restore_height: u64,
    seed_offset: &str,
) -> Result<RestoreWalletResponse, String> {
    with_wallet(handle, |w| {
        let val = w
            .restore_deterministic_wallet(
                wallet_path,
                seed,
                password,
                language,
                restore_height,
                seed_offset,
            )
            .map_err(engine_err)?;
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
    wallet_path: &str,
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
                wallet_path,
                address,
                spendkey,
                viewkey,
                password,
                language,
                restore_height,
            )
            .map_err(engine_err)?;
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
        let val = w.get_address(account_index).map_err(engine_err)?;
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
        let val = w.get_balance(account_index).map_err(engine_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn query_key(handle: &WalletHandle, key_type: &str) -> Result<String, String> {
    with_wallet(handle, |w| {
        let val = w.query_key(key_type).map_err(engine_err)?;
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
            .map_err(engine_err)?;
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
            .map_err(engine_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

#[allow(dead_code)]
pub fn stop_wallet(handle: &WalletHandle) -> Result<(), String> {
    with_wallet(handle, |w| w.stop().map_err(engine_err))
}

// ─── Staking ─────────────────────────────────────────────────────────────────

pub fn stake(handle: &WalletHandle, tier: u8, amount: u64) -> Result<TransferResponse, String> {
    with_wallet(handle, |w| {
        let params = serde_json::json!({ "tier": tier, "amount": amount });
        let val = w
            .json_rpc_call("stake", &params.to_string())
            .map_err(engine_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

pub fn claim_rewards(handle: &WalletHandle) -> Result<TransferResponse, String> {
    with_wallet(handle, |w| {
        let val = w.json_rpc_call("claim_rewards", "{}").map_err(engine_err)?;
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
            .map_err(engine_err)?;
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
            .map_err(engine_err)?;
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
            .map_err(engine_err)?;
        serde_json::from_value(val).map_err(|e| format!("Parse error: {e}"))
    })
}

// ─── Scanner-backed queries ──────────────────────────────────────────────────

/// Get balance from the Rust scanner state.
pub async fn get_scanner_balance(
    handle: &WalletHandle,
) -> Result<shekyl_scanner::BalanceSummary, String> {
    let state_arc = scanner_state(handle)?;
    let guard = state_arc.lock().await;
    let (ledger, _indexes) = &*guard;
    let height = ledger.height();
    Ok(ledger.balance(height))
}

/// Get staked outputs from the Rust scanner state.
///
/// Unavailable until core lands `LedgerBlockExt::stake_views` /
/// `feat/scanner-stake-views`. Returning an empty list would be
/// indistinguishable from "wallet has no stakes" (rule 82) — fail loud.
pub async fn get_scanner_staked_outputs(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let _ = scanner_state(handle)?;
    Err(
        "scanner stake views not available yet (awaiting core StakeView / feat/scanner-stake-views)"
            .into(),
    )
}

/// Get claimable staked outputs from the Rust scanner state.
///
/// See [`get_scanner_staked_outputs`] — fail loud until StakeView lands.
pub async fn get_scanner_claimable_stakes(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let _ = scanner_state(handle)?;
    Err(
        "scanner stake views not available yet (awaiting core StakeView / feat/scanner-stake-views)"
            .into(),
    )
}

/// Get unstakeable (matured) outputs from the Rust scanner state.
///
/// See [`get_scanner_staked_outputs`] — fail loud until StakeView lands.
pub async fn get_scanner_unstakeable_outputs(
    handle: &WalletHandle,
) -> Result<serde_json::Value, String> {
    let _ = scanner_state(handle)?;
    Err(
        "scanner stake views not available yet (awaiting core StakeView / feat/scanner-stake-views)"
            .into(),
    )
}

/// Freeze an output by key image via the scanner state.
pub async fn scanner_freeze(handle: &WalletHandle, key_image_hex: &str) -> Result<bool, String> {
    let ki = parse_key_image(key_image_hex)?;
    let state_arc = scanner_state(handle)?;
    let mut guard = state_arc.lock().await;
    let (ledger, indexes) = &mut *guard;
    Ok(indexes.freeze_by_key_image(ledger, &ki))
}

/// Thaw a frozen output by key image via the scanner state.
pub async fn scanner_thaw(handle: &WalletHandle, key_image_hex: &str) -> Result<bool, String> {
    let ki = parse_key_image(key_image_hex)?;
    let state_arc = scanner_state(handle)?;
    let mut guard = state_arc.lock().await;
    let (ledger, indexes) = &mut *guard;
    Ok(indexes.thaw_by_key_image(ledger, &ki))
}

/// Get the scanner's synced height.
pub async fn get_scanner_height(handle: &WalletHandle) -> Result<u64, String> {
    let state_arc = scanner_state(handle)?;
    let guard = state_arc.lock().await;
    let (ledger, _indexes) = &*guard;
    Ok(ledger.height())
}

fn parse_key_image(hex_str: &str) -> Result<KeyImage, String> {
    if hex_str.len() != 64 {
        return Err(format!(
            "key_image must be 64 hex chars, got {}",
            hex_str.len()
        ));
    }
    let bytes = decode_hex_32(hex_str)?;
    Ok(KeyImage::from_canonical_bytes(bytes))
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
            .map_err(engine_err)?;
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
        let (ledger, _indexes) = &*state;
        assert_eq!(
            ledger.height(),
            0,
            "scanner state height must be preserved (0) after transfer error"
        );
        assert_eq!(
            ledger.transfers().len(),
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
        let (ledger, _indexes) = &*state;
        assert_eq!(
            ledger.height(),
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
    // `shekyl-engine-state`'s ledger-index test suite.
}
