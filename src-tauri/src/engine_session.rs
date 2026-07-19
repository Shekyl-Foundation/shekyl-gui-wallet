// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Pure-Rust Engine wallet session (GUI-PR1).
//!
//! Embeds [`shekyl_engine_core::Engine`] directly — no `wallet2_ffi`.
//! Mirrors the embedder choreography in `shekyl-wallet-rpc` lifecycle
//! (create / open / close / P-scan wrap) without the HTTP JSON-RPC layer.
//!
//! Wallet files use the Engine envelope pair:
//! `{name}.wallet` + `{name}.wallet.keys` under the configured wallet dir.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::rngs::OsRng;
use rand::RngCore;
use shekyl_crypto_pq::account::{
    generate_account_from_bip39, generate_account_from_raw_seed, DerivationNetwork, SeedFormat,
    MASTER_SEED_BYTES, RAW_SEED_BYTES,
};
use shekyl_crypto_pq::bip39::{mnemonic_from_entropy, SHEKYL_BIP39_ENTROPY_BYTES};
use shekyl_crypto_pq::wallet_envelope::KdfParams;
use shekyl_engine_core::{
    Credentials, DaemonClient, Engine, EngineCreateParams, Network, OpenedEngine, PScanHandle,
    RefreshOptions, SoloSigner,
};
use shekyl_engine_file::paths::keys_path_from;
use shekyl_engine_file::SafetyOverrides;
use shekyl_engine_prefs::WalletPrefs;
use shekyl_rpc_transport::SimpleRequestRpc;
use shekyl_scanner::LedgerBlockExt;
use tokio::sync::RwLock;
use tracing::warn;
use zeroize::{Zeroize, Zeroizing};

use crate::state::NetworkType;

/// Shared Engine handle (same shape as wallet-rpc `SharedEngine`).
pub type SharedEngine = Arc<RwLock<Engine<SoloSigner>>>;

/// In-process Engine session for the desktop wallet.
pub struct EngineSession {
    engine: Option<SharedEngine>,
    pscan: Option<PScanHandle>,
    name: Option<String>,
    /// One-shot mnemonic retained only until the create response is
    /// delivered (Engine drops seed material at open; mid-session
    /// `get_seed` cannot re-materialize it without a password reopen).
    create_mnemonic: Option<String>,
}

impl EngineSession {
    pub fn new() -> Self {
        Self {
            engine: None,
            pscan: None,
            name: None,
            create_mnemonic: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.engine.is_some()
    }

    #[allow(dead_code)] // used by future stake activation reopen
    pub fn open_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a fresh Engine wallet (BIP-39 on mainnet/stagenet; raw32 on testnet).
    pub async fn create(
        &mut self,
        wallet_dir: &Path,
        name: &str,
        password: &str,
        network: NetworkType,
        daemon_http_base: &str,
    ) -> Result<CreateOutcome, String> {
        if self.engine.is_some() {
            return Err("A wallet is already open".into());
        }

        let base = engine_wallet_base(wallet_dir, name);
        if keys_path_from(&base).exists() {
            return Err("Wallet file already exists".into());
        }
        std::fs::create_dir_all(wallet_dir)
            .map_err(|e| format!("Failed to create wallet directory: {e}"))?;

        let password = Zeroizing::new(password.as_bytes().to_vec());
        let engine_net = map_network(network);
        let daemon = make_daemon(daemon_http_base).await?;

        let (master_seed, seed_format, backup) = generate_seed_material(engine_net)?;
        let creation_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let creds = Credentials::password_only(password.as_slice());
        let create_params = EngineCreateParams {
            base_path: &base,
            credentials: &creds,
            network: engine_net,
            capability: shekyl_engine_core::CapabilityInput::Full {
                master_seed_64: &master_seed,
                seed_format,
            },
            creation_timestamp,
            restore_height_hint: 0,
            kdf: KdfParams::default(),
            overrides: SafetyOverrides::none(),
            prefs: WalletPrefs::default(),
        };

        let engine = tokio::task::block_in_place(|| {
            Engine::<SoloSigner>::create(create_params, daemon)
        })
        .map_err(map_open_err)?;
        drop(master_seed);

        let address = engine
            .primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))?;

        let (shared, pscan) = wrap_and_start_pscan(engine).await?;
        self.engine = Some(shared);
        self.pscan = pscan;
        self.name = Some(name.to_owned());

        let seed = match backup {
            SeedBackup::Mnemonic(m) => {
                self.create_mnemonic = Some(m.clone());
                m
            }
            SeedBackup::RawHex(h) => {
                // Testnet raw seed: surface as hex for backup; not BIP-39.
                self.create_mnemonic = None;
                h
            }
        };

        Ok(CreateOutcome { address, seed })
    }

    /// Restore an Engine wallet from a BIP-39 mnemonic (mainnet/stagenet).
    pub async fn restore_from_bip39(
        &mut self,
        wallet_dir: &Path,
        name: &str,
        mnemonic: &str,
        password: &str,
        passphrase: &str,
        restore_height: u64,
        network: NetworkType,
        daemon_http_base: &str,
    ) -> Result<String, String> {
        if self.engine.is_some() {
            return Err("A wallet is already open".into());
        }
        let engine_net = map_network(network);
        if matches!(engine_net, Network::Testnet) {
            return Err(
                "BIP-39 restore is for mainnet/stagenet; testnet uses raw seeds".into(),
            );
        }

        let base = engine_wallet_base(wallet_dir, name);
        if keys_path_from(&base).exists() {
            return Err("Wallet file already exists".into());
        }
        std::fs::create_dir_all(wallet_dir)
            .map_err(|e| format!("Failed to create wallet directory: {e}"))?;

        let derivation = network_to_derivation(engine_net);
        let (master_seed, _blob) =
            generate_account_from_bip39(mnemonic, passphrase, derivation)
                .map_err(|e| format!("BIP-39 restore failed: {e}"))?;

        let password = Zeroizing::new(password.as_bytes().to_vec());
        let daemon = make_daemon(daemon_http_base).await?;
        let creation_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let creds = Credentials::password_only(password.as_slice());
        let create_params = EngineCreateParams {
            base_path: &base,
            credentials: &creds,
            network: engine_net,
            capability: shekyl_engine_core::CapabilityInput::Full {
                master_seed_64: &master_seed,
                seed_format: SeedFormat::Bip39,
            },
            creation_timestamp,
            restore_height_hint: u32::try_from(restore_height).unwrap_or(u32::MAX),
            kdf: KdfParams::default(),
            overrides: SafetyOverrides::none(),
            prefs: WalletPrefs::default(),
        };

        let engine = tokio::task::block_in_place(|| {
            Engine::<SoloSigner>::create(create_params, daemon)
        })
        .map_err(map_open_err)?;
        drop(master_seed);

        let address = engine
            .primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))?;

        let (shared, pscan) = wrap_and_start_pscan(engine).await?;
        self.engine = Some(shared);
        self.pscan = pscan;
        self.name = Some(name.to_owned());
        self.create_mnemonic = None;

        Ok(address)
    }

    /// Open an existing Engine wallet.
    pub async fn open(
        &mut self,
        wallet_dir: &Path,
        name: &str,
        password: &str,
        network: NetworkType,
        daemon_http_base: &str,
    ) -> Result<String, String> {
        if self.engine.is_some() {
            return Err("A wallet is already open".into());
        }

        let base = engine_wallet_base(wallet_dir, name);
        if !keys_path_from(&base).exists() {
            return Err("Wallet file not found".into());
        }

        let password = Zeroizing::new(password.as_bytes().to_vec());
        let engine_net = map_network(network);
        let daemon = make_daemon(daemon_http_base).await?;
        let creds = Credentials::password_only(password.as_slice());

        let opened = tokio::task::block_in_place(|| {
            Engine::<SoloSigner>::open_full(
                &base,
                &creds,
                engine_net,
                daemon,
                SafetyOverrides::none(),
            )
        })
        .map_err(map_open_err)?;

        let engine = match opened {
            OpenedEngine::Loaded(w) => w,
            OpenedEngine::Restored { wallet, .. } => wallet,
        };

        let address = engine
            .primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))?;

        let (shared, pscan) = wrap_and_start_pscan(engine).await?;
        self.engine = Some(shared);
        self.pscan = pscan;
        self.name = Some(name.to_owned());
        self.create_mnemonic = None;

        Ok(address)
    }

    /// Persist and close the open Engine wallet.
    pub async fn close(&mut self) -> Result<(), String> {
        let Some(shared) = self.engine.take() else {
            self.pscan = None;
            self.name = None;
            self.create_mnemonic = None;
            return Ok(());
        };
        if let Some(handle) = self.pscan.take() {
            handle.shutdown().await;
        }
        let lock = Arc::try_unwrap(shared).map_err(|_| {
            "cannot close: wallet engine still in use by another task".to_string()
        })?;
        let engine = lock.into_inner();
        tokio::task::block_in_place(|| engine.persist_for_close()).map_err(map_open_err)?;
        drop(engine);
        self.name = None;
        self.create_mnemonic = None;
        Ok(())
    }

    /// Run a one-shot refresh (blocks until complete).
    pub async fn refresh(&self) -> Result<(), String> {
        let shared = self
            .engine
            .clone()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let handle = Engine::start_refresh(shared, RefreshOptions::default())
            .await
            .map_err(|e| format!("refresh: {e}"))?;
        handle.join().await.map_err(|e| format!("refresh: {e}"))?;
        Ok(())
    }

    pub async fn primary_address(&self) -> Result<String, String> {
        let shared = self
            .engine
            .as_ref()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let g = shared.read().await;
        g.primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))
    }

    /// Balance as total / unlocked / staked (staked always 0 until Stage 3).
    pub async fn balance(&self) -> Result<(u64, u64, u64), String> {
        let shared = self
            .engine
            .as_ref()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let g = shared.read().await;
        let ledger = g.ledger();
        let height = ledger.ledger.height();
        let summary = ledger.ledger.balance(height);
        Ok((
            summary.total.to_raw(),
            summary.unlocked.to_raw(),
            0, // staked — Stage 3
        ))
    }

    /// Seed available only immediately after create (if BIP-39 path).
    pub fn take_create_mnemonic(&mut self) -> Option<String> {
        self.create_mnemonic.take()
    }

    /// Mid-session seed is not available on the Engine path (seed dropped at open).
    pub fn seed_unavailable_message() -> &'static str {
        "recovery phrase is only shown once at wallet creation on the Engine backend; \
         mid-session seed display requires a credentialed reopen (not yet exposed)"
    }
}

impl Default for EngineSession {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CreateOutcome {
    pub address: String,
    pub seed: String,
}

/// Base path `{wallet_dir}/{name}.wallet` for Engine file envelope.
pub fn engine_wallet_base(wallet_dir: &Path, name: &str) -> PathBuf {
    wallet_dir.join(format!("{name}.wallet"))
}

/// True if an Engine wallet keys file exists for `name`.
pub fn engine_wallet_exists(wallet_dir: &Path, name: &str) -> bool {
    keys_path_from(&engine_wallet_base(wallet_dir, name)).exists()
}

fn map_network(n: NetworkType) -> Network {
    match n {
        NetworkType::Mainnet => Network::Mainnet,
        NetworkType::Testnet => Network::Testnet,
        NetworkType::Stagenet => Network::Stagenet,
    }
}

fn network_to_derivation(network: Network) -> DerivationNetwork {
    match network {
        Network::Mainnet => DerivationNetwork::Mainnet,
        Network::Testnet => DerivationNetwork::Testnet,
        Network::Stagenet => DerivationNetwork::Stagenet,
    }
}

enum SeedBackup {
    Mnemonic(String),
    RawHex(String),
}

fn generate_seed_material(
    network: Network,
) -> Result<(Zeroizing<[u8; MASTER_SEED_BYTES]>, SeedFormat, SeedBackup), String> {
    let derivation = network_to_derivation(network);
    match network {
        Network::Mainnet | Network::Stagenet => {
            let mut entropy = [0u8; SHEKYL_BIP39_ENTROPY_BYTES];
            OsRng.fill_bytes(&mut entropy);
            let mnemonic = mnemonic_from_entropy(&entropy)
                .map_err(|e| format!("mnemonic_from_entropy: {e}"))?;
            entropy.zeroize();
            let (master, _blob) = generate_account_from_bip39(&mnemonic, "", derivation)
                .map_err(|e| format!("bip39 account: {e}"))?;
            Ok((master, SeedFormat::Bip39, SeedBackup::Mnemonic(mnemonic)))
        }
        Network::Testnet => {
            let mut raw = [0u8; RAW_SEED_BYTES];
            OsRng.fill_bytes(&mut raw);
            let seed_hex = hex::encode(raw);
            let (master, _blob) = generate_account_from_raw_seed(&raw, derivation)
                .map_err(|e| format!("raw account: {e}"))?;
            raw.zeroize();
            Ok((master, SeedFormat::Raw32, SeedBackup::RawHex(seed_hex)))
        }
    }
}

async fn make_daemon(daemon_http_base: &str) -> Result<DaemonClient, String> {
    let url = daemon_http_base
        .trim_end_matches("/json_rpc")
        .trim_end_matches('/')
        .to_owned();
    let rpc = SimpleRequestRpc::new(url)
        .await
        .map_err(|e| format!("daemon unreachable: {e}"))?;
    Ok(DaemonClient::new(rpc))
}

async fn wrap_and_start_pscan(
    engine: Engine<SoloSigner>,
) -> Result<(SharedEngine, Option<PScanHandle>), String> {
    let shared: SharedEngine = Arc::new(RwLock::new(engine));
    match Engine::start_pscan_if_staker(shared.clone()).await {
        Ok(handle) => Ok((shared, handle)),
        Err(e) => {
            warn!(
                error = %e,
                "staker P-scan failed to start; aborting open (fail-closed)"
            );
            Err(format!(
                "staker P-scan failed to load; open aborted (fail-closed): {e}"
            ))
        }
    }
}

fn map_open_err(e: shekyl_engine_core::OpenError) -> String {
    format!("wallet error: {e}")
}

/// Whether the process should prefer the Engine backend.
///
/// Enabled when `SHEKYL_ENGINE_BACKEND=1` (or `true` / `yes`), or when
/// the env var is unset and the compile-time default is used.
/// Default: **true** (Rust-forward mainline). Set `SHEKYL_ENGINE_BACKEND=0`
/// to force the transitional Wallet2 path.
pub fn engine_backend_default_from_env() -> bool {
    match std::env::var("SHEKYL_ENGINE_BACKEND") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_wallet_base_appends_wallet_suffix() {
        let p = engine_wallet_base(Path::new("/tmp/wallets"), "Alice");
        assert_eq!(p, PathBuf::from("/tmp/wallets/Alice.wallet"));
    }

    #[test]
    fn env_default_parses() {
        // Just ensure the function does not panic without the env set.
        let _ = engine_backend_default_from_env();
    }
}
