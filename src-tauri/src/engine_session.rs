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

use std::collections::HashMap;
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
use shekyl_engine_core::engine::SubmitError;
use shekyl_engine_core::{
    Capability, Credentials, DaemonClient, DrainBalanceReadError, Engine, EngineCreateParams,
    FeePriority, FirstStakeError, FirstStakeOutcome, Network, OpenedEngine, PScanHandle,
    RefreshOptions, SoloSigner, TxRecipient, TxRequest,
};
use shekyl_engine_file::paths::keys_path_from;
use shekyl_engine_file::{SafetyOverrides, WalletFile};
use shekyl_engine_prefs::WalletPrefs;
use shekyl_engine_state::{SendRecord, SendState};
use shekyl_rpc_transport::HttpRpc;
use shekyl_scanner::LedgerBlockExt;
use shekyl_units::AtomicUnits;
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
    /// Engine file base (`…/{name}.wallet`) for credentialed reopen.
    base_path: Option<PathBuf>,
    network: Option<Network>,
    daemon_http_base: Option<String>,
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
            base_path: None,
            network: None,
            daemon_http_base: None,
            create_mnemonic: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.engine.is_some()
    }

    fn remember_open(
        &mut self,
        name: &str,
        base: PathBuf,
        network: Network,
        daemon_http_base: &str,
        shared: SharedEngine,
        pscan: Option<PScanHandle>,
    ) {
        self.engine = Some(shared);
        self.pscan = pscan;
        self.name = Some(name.to_owned());
        self.base_path = Some(base);
        self.network = Some(network);
        self.daemon_http_base = Some(daemon_http_base.to_owned());
    }

    fn clear_open(&mut self) {
        self.engine = None;
        self.pscan = None;
        self.name = None;
        self.base_path = None;
        self.network = None;
        self.daemon_http_base = None;
        self.create_mnemonic = None;
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

        let engine =
            tokio::task::block_in_place(|| Engine::<SoloSigner>::create(create_params, daemon))
                .map_err(map_open_err)?;
        drop(master_seed);

        let address = engine
            .primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))?;

        let (shared, pscan) = wrap_and_start_pscan(engine).await?;
        self.remember_open(name, base, engine_net, daemon_http_base, shared, pscan);

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
    #[allow(clippy::too_many_arguments)]
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
            return Err("BIP-39 restore is for mainnet/stagenet; testnet uses raw seeds".into());
        }

        let base = engine_wallet_base(wallet_dir, name);
        if keys_path_from(&base).exists() {
            return Err("Wallet file already exists".into());
        }
        std::fs::create_dir_all(wallet_dir)
            .map_err(|e| format!("Failed to create wallet directory: {e}"))?;

        let derivation = network_to_derivation(engine_net);
        let (master_seed, _blob) = generate_account_from_bip39(mnemonic, passphrase, derivation)
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

        let engine =
            tokio::task::block_in_place(|| Engine::<SoloSigner>::create(create_params, daemon))
                .map_err(map_open_err)?;
        drop(master_seed);

        let address = engine
            .primary_address()
            .encode()
            .map_err(|e| format!("encode address: {e}"))?;

        let (shared, pscan) = wrap_and_start_pscan(engine).await?;
        self.remember_open(name, base, engine_net, daemon_http_base, shared, pscan);
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
        self.remember_open(name, base, engine_net, daemon_http_base, shared, pscan);
        self.create_mnemonic = None;

        Ok(address)
    }

    /// Persist and close the open Engine wallet.
    pub async fn close(&mut self) -> Result<(), String> {
        let Some(shared) = self.engine.take() else {
            self.clear_open();
            return Ok(());
        };
        if let Some(handle) = self.pscan.take() {
            handle.shutdown().await;
        }
        let lock = Arc::try_unwrap(shared)
            .map_err(|_| "cannot close: wallet engine still in use by another task".to_string())?;
        let engine = lock.into_inner();
        tokio::task::block_in_place(|| engine.persist_for_close()).map_err(map_open_err)?;
        drop(engine);
        self.clear_open();
        Ok(())
    }

    /// Archival staker status for the Staking page.
    pub async fn staker_status(&self) -> Result<StakerStatus, String> {
        let shared = self
            .engine
            .as_ref()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let g = shared.read().await;
        let staking = &g.ledger().staking;
        Ok(StakerStatus {
            staking_enabled: staking.staking_enabled,
            has_stake_engine: g.has_stake_engine(),
            bonded_slot_count: staking.bonded_slots.len() as u32,
            has_pscan: self.pscan.is_some(),
        })
    }

    /// Become a staker: credentialed first-stake activation (GUI-PR3).
    ///
    /// Mirrors `shekyl-wallet-rpc` `stake { password }`: verify password →
    /// optional intent reopen → `Engine::first_stake`. No broadcast on this
    /// path (`state: pending_dispatch`).
    pub async fn activate_staker(
        &mut self,
        password: &str,
    ) -> Result<ActivateStakerOutcome, String> {
        let shared = self
            .engine
            .clone()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let base = self
            .base_path
            .clone()
            .ok_or_else(|| "Engine session missing wallet path".to_string())?;
        let network = self
            .network
            .ok_or_else(|| "Engine session missing network".to_string())?;
        let daemon_base = self
            .daemon_http_base
            .clone()
            .ok_or_else(|| "Engine session missing daemon address".to_string())?;
        let name = self
            .name
            .clone()
            .ok_or_else(|| "Engine session missing wallet name".to_string())?;

        let password_z = Zeroizing::new(password.as_bytes().to_vec());

        let (needs_intent_open, slot) = {
            let g = shared.read().await;
            if g.capability() != Capability::Full {
                return Err("staking requires a full-capability wallet".into());
            }
            let staking = &g.ledger().staking;
            let slot = if staking.staking_enabled {
                staking
                    .bonded_slots
                    .first()
                    .copied()
                    .unwrap_or_else(|| staking.monotone_current_slot_from_record())
            } else {
                staking.monotone_current_slot_from_record()
            };
            let has_scan = self.pscan.is_some();
            (!g.has_stake_engine() || !has_scan, slot)
        };

        let shared = if needs_intent_open {
            drop(shared);
            self.reopen_with_first_stake_intent(
                &base,
                network,
                &daemon_base,
                &name,
                password_z,
                slot,
            )
            .await?
        } else {
            drop(password_z);
            shared
        };

        let outcome = Engine::first_stake(shared, slot)
            .await
            .map_err(map_first_stake_err)?;

        Ok(ActivateStakerOutcome::from(outcome))
    }

    /// SA-R1-a: verify password, close, reopen with first-stake intent, start P-scan.
    async fn reopen_with_first_stake_intent(
        &mut self,
        base: &Path,
        network: Network,
        daemon_http_base: &str,
        name: &str,
        password: Zeroizing<Vec<u8>>,
        slot: u32,
    ) -> Result<SharedEngine, String> {
        // Verify-then-close: wrong password refuses with wallet still open.
        tokio::task::block_in_place(|| WalletFile::verify_password(base, password.as_slice()))
            .map_err(|e| match e {
                shekyl_engine_file::WalletFileError::Envelope(_) => {
                    "incorrect password".to_string()
                }
                other => format!("password verification failed: {other}"),
            })?;

        let daemon = make_daemon(daemon_http_base).await?;

        // Close current session (persist + scan shutdown).
        let shared = self
            .engine
            .take()
            .ok_or_else(|| "No wallet is open".to_string())?;
        if let Some(handle) = self.pscan.take() {
            handle.shutdown().await;
        }
        let lock = Arc::try_unwrap(shared).map_err(|_| {
            "cannot re-open for staking: wallet engine still in use by another task".to_string()
        })?;
        let engine = lock.into_inner();
        if let Err(e) = tokio::task::block_in_place(|| engine.persist_for_close()) {
            // Restore open without intent.
            let shared = Arc::new(RwLock::new(engine));
            let pscan = restart_pscan(&shared).await;
            self.remember_open(
                name,
                base.to_path_buf(),
                network,
                daemon_http_base,
                shared,
                pscan,
            );
            return Err(format!("could not close for stake activation: {e}"));
        }
        drop(engine);

        let reopened = tokio::task::block_in_place(|| {
            let creds = Credentials::password_only(password.as_slice());
            Engine::<SoloSigner>::open_full_with_first_stake_intent(
                base,
                &creds,
                network,
                daemon,
                SafetyOverrides::none(),
                slot,
            )
        });

        let engine = match reopened {
            Ok(OpenedEngine::Loaded(w)) | Ok(OpenedEngine::Restored { wallet: w, .. }) => w,
            Err(e) => {
                // Best-effort plain reopen so the user is not logged out.
                let restore = async {
                    let pw = Zeroizing::new(password.as_slice().to_vec());
                    let daemon = make_daemon(daemon_http_base).await?;
                    let creds = Credentials::password_only(pw.as_slice());
                    let opened = tokio::task::block_in_place(|| {
                        Engine::<SoloSigner>::open_full(
                            base,
                            &creds,
                            network,
                            daemon,
                            SafetyOverrides::none(),
                        )
                    })
                    .map_err(map_open_err)?;
                    Ok::<_, String>(match opened {
                        OpenedEngine::Loaded(w) | OpenedEngine::Restored { wallet: w, .. } => w,
                    })
                }
                .await;
                match restore {
                    Ok(w) => {
                        let (shared, pscan) = wrap_and_start_pscan(w).await?;
                        self.remember_open(
                            name,
                            base.to_path_buf(),
                            network,
                            daemon_http_base,
                            shared,
                            pscan,
                        );
                        return Err(format!(
                            "stake activation reopen failed ({e}); wallet remains open — retry"
                        ));
                    }
                    Err(restore_err) => {
                        self.clear_open();
                        return Err(format!(
                            "stake activation reopen failed ({e}) and restore failed ({restore_err}); \
                             open the wallet again"
                        ));
                    }
                }
            }
        };

        // On-demand P-scan under intent (fail-closed if dark).
        let shared: SharedEngine = Arc::new(RwLock::new(engine));
        match Engine::start_pscan(shared.clone()).await {
            Ok(handle) => {
                self.remember_open(
                    name,
                    base.to_path_buf(),
                    network,
                    daemon_http_base,
                    shared.clone(),
                    Some(handle),
                );
                Ok(shared)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "first-stake intent reopen: P-scan failed; wallet open without scan"
                );
                self.remember_open(
                    name,
                    base.to_path_buf(),
                    network,
                    daemon_http_base,
                    shared,
                    None,
                );
                Err(format!(
                    "persona scan failed to start ({e}); wallet remains open — retry activation"
                ))
            }
        }
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

    /// F-D2 aggregate drainable-`P` read (DS-PR-3 PR-B; `ARCHIVAL_DRAIN_SEND_FD2.md`
    /// §1 layer 1). Mirrors [`Self::refresh`]'s arc-clone shape: clone the engine
    /// arc and drive the self-arc accessor `Engine::drain_balance_aggregate`,
    /// which anchors the same send-path reference a real drain proves against.
    ///
    /// Preserves the core two-armed [`DrainBalanceReadError`] split across the
    /// command boundary (the DS-PR-3 locked decision, rule 82): the genuinely
    /// transient `Unanchorable` arm becomes [`DrainBalanceRead::Syncing`] (the UI
    /// renders "syncing", never a zero), while a non-transient `State` fault stays
    /// an `Err(String)` the frontend catches and renders as "—", never a
    /// fabricated zero. A non-staker / unscanned wallet is an honest
    /// [`DrainBalanceRead::Ready`] of `0` (the core accessor short-circuits to
    /// `Ok(0)` before anchoring).
    pub async fn drain_balance(&self) -> Result<DrainBalanceRead, String> {
        let shared = self
            .engine
            .clone()
            .ok_or_else(|| "No wallet is open".to_string())?;
        match Engine::drain_balance_aggregate(shared).await {
            Ok(spendable) => Ok(DrainBalanceRead::Ready {
                spendable: spendable.to_raw(),
            }),
            Err(DrainBalanceReadError::Unanchorable { detail }) => Ok(DrainBalanceRead::Syncing {
                detail: detail.to_string(),
            }),
            Err(DrainBalanceReadError::State { detail }) => Err(format!("drain balance: {detail}")),
        }
    }

    /// One-shot send: build pending tx + submit (GUI transfer command).
    ///
    /// Mirrors wallet-rpc `build_pending_tx` → `submit_pending_tx` with
    /// `FeePriority::Standard`. On CT-5d `ContentChanged`, resubmits once
    /// with the advanced `content_gen` (user already confirmed the send
    /// intent at the UI layer for this one-shot path).
    pub async fn transfer(
        &self,
        address: &str,
        amount_atomic: u64,
    ) -> Result<TransferOutcome, String> {
        let shared = self
            .engine
            .clone()
            .ok_or_else(|| "No wallet is open".to_string())?;

        let request = TxRequest {
            recipients: vec![TxRecipient {
                address: address.to_owned(),
                amount_atomic_units: AtomicUnits::from_raw(amount_atomic),
            }],
            priority: FeePriority::Standard,
        };

        // Phase 4b: build/submit/discard take `&self` (interior mutability +
        // engine-owned permits). Hold a *read* guard — same as wallet-rpc —
        // so P-scan and other SharedEngine readers are not stalled across
        // FCMP++ assembly and the daemon submit RTT. Serialization of the
        // send path itself lives in LocalPendingTx, not this lock.
        let engine = shared.read().await;
        let pending = engine
            .build_pending_tx_async(&request)
            .await
            .map_err(|e| format!("build transfer: {e}"))?;

        let fee = pending.fee_atomic_units.to_raw();
        let id = pending.id;
        let mut seen_gen = pending.content_gen;

        // SubmitOutcome is identity-bearing (Accepted / AlreadyInPool /
        // AlreadyInChain); one-shot GUI needs only the txid. Verdict UX is a
        // separate follow-up; refresh remains settlement authority.
        let tx_hash = match engine.submit_pending_tx_async(id, seen_gen).await {
            Ok(outcome) => outcome.hash(),
            Err(SubmitError::ContentChanged {
                content_gen,
                reservation_id,
            }) => {
                // One-shot GUI path: re-confirm is implicit; resubmit once.
                seen_gen = content_gen;
                engine
                    .submit_pending_tx_async(reservation_id, seen_gen)
                    .await
                    .map_err(|e| {
                        // Best-effort discard so funds unlock if still held.
                        let _ = engine.discard_pending_tx(reservation_id);
                        format!("submit transfer (after re-anchor): {e}")
                    })?
                    .hash()
            }
            Err(e) => {
                let _ = engine.discard_pending_tx(id);
                return Err(format!("submit transfer: {e}"));
            }
        };

        Ok(TransferOutcome {
            tx_hash: tx_hash.to_string(),
            amount: amount_atomic,
            fee,
        })
    }

    /// Estimate fee by building (then discarding) a pending tx.
    pub async fn estimate_fee(&self, address: &str, amount_atomic: u64) -> Result<u64, String> {
        let shared = self
            .engine
            .clone()
            .ok_or_else(|| "No wallet is open".to_string())?;

        let request = TxRequest {
            recipients: vec![TxRecipient {
                address: address.to_owned(),
                amount_atomic_units: AtomicUnits::from_raw(amount_atomic),
            }],
            priority: FeePriority::Standard,
        };

        // Read guard: fee estimate is build+discard under the same Phase 4b
        // shared-borrow contract as transfer (see above).
        let engine = shared.read().await;
        let pending = engine
            .build_pending_tx_async(&request)
            .await
            .map_err(|e| format!("fee estimate: {e}"))?;
        let fee = pending.fee_atomic_units.to_raw();
        let _ = engine.discard_pending_tx(pending.id);
        Ok(fee)
    }

    /// Project receive ledger + send journal into a transaction list.
    ///
    /// Mirrors wallet-rpc `get_transfers` (PR-SJ-2): **incoming** rows fold
    /// scan-ledger outputs by creating txid; **outgoing** rows project
    /// `send_journal` records with distinct status per lifecycle arm
    /// (pending / confirmed / failed / dropped — rule 82, never collapse).
    ///
    /// Inclusion height is `0` when the send was never mined (dispatched,
    /// failed, or dropped) — projecting `dispatched_at_height` would put a
    /// plausible block number beside a payment that does not exist on chain.
    pub async fn list_transfers(&self) -> Result<Vec<TransferRow>, String> {
        let shared = self
            .engine
            .as_ref()
            .ok_or_else(|| "No wallet is open".to_string())?;
        let g = shared.read().await;
        let ledger = g.ledger();
        let tip = ledger.ledger.height();
        merge_transfer_history(ledger.ledger.transfers(), &ledger.send_journal.rows, tip)
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

/// Result of a one-shot Engine transfer.
pub struct TransferOutcome {
    pub tx_hash: String,
    pub amount: u64,
    pub fee: u64,
}

/// Ledger / journal row projected for the transactions list.
pub struct TransferRow {
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    /// Inclusion height, or `0` when the tx is not on chain.
    pub height: u64,
    pub timestamp: u64,
    /// `"in"` (scan ledger) or `"out"` (send journal).
    pub direction: String,
    /// Lifecycle status: `confirmed` | `pending` | `failed` | `dropped`.
    ///
    /// Every arm is a distinct user-facing situation (rule 82); failed and
    /// dropped must never render as confirmed or pending.
    pub status: String,
    /// `true` iff [`Self::status`] is `"confirmed"` — kept for callers that
    /// only need the binary settlement bit.
    pub confirmed: bool,
    pub pqc_protected: bool,
}

/// Merge scan-ledger receipts and send-journal sends into one newest-first list.
///
/// Pure so unit tests can exercise the projection without an open Engine.
fn merge_transfer_history(
    transfers: &[shekyl_engine_state::TransferDetails],
    journal_rows: &std::collections::BTreeMap<[u8; 32], SendRecord>,
    tip: u64,
) -> Result<Vec<TransferRow>, String> {
    let mut rows: Vec<TransferRow> = Vec::new();

    // Incoming: fold received outputs into one row per creating transaction.
    let mut order: Vec<String> = Vec::new();
    let mut by_tx: HashMap<String, TransferRow> = HashMap::new();
    for td in transfers {
        let hash = td.tx_hash.to_string();
        let amount = td.amount().to_raw();
        if let Some(row) = by_tx.get_mut(&hash) {
            row.amount = row
                .amount
                .checked_add(amount)
                .ok_or_else(|| "transaction receipt total overflowed u64".to_string())?;
            continue;
        }
        let confirmed = td.block_height > 0 && tip >= td.block_height;
        let status = if confirmed {
            "confirmed".to_owned()
        } else {
            "pending".to_owned()
        };
        order.push(hash.clone());
        by_tx.insert(
            hash.clone(),
            TransferRow {
                hash,
                amount,
                fee: 0,
                height: td.block_height,
                timestamp: 0,
                direction: "in".into(),
                status: status.clone(),
                confirmed,
                pqc_protected: true,
            },
        );
    }
    rows.extend(order.into_iter().filter_map(|h| by_tx.remove(&h)));

    // Outgoing: one row per send-journal record (PR-SJ-2 parity).
    for (txid, record) in journal_rows {
        rows.push(project_outgoing_row(txid, record)?);
    }

    // Newest first: never-mined / mempool (height 0) at the top, then
    // descending inclusion height. Within a height, incoming before outgoing
    // (wallet-rpc order), then hash for stability.
    rows.sort_by(|a, b| {
        let key = |h: u64| if h == 0 { u64::MAX } else { h };
        key(b.height)
            .cmp(&key(a.height))
            .then_with(|| {
                let a_out = a.direction == "out";
                let b_out = b.direction == "out";
                a_out.cmp(&b_out)
            })
            .then_with(|| a.hash.cmp(&b.hash))
    });
    Ok(rows)
}

/// Project one send-journal record as an outgoing [`TransferRow`].
fn project_outgoing_row(txid: &[u8; 32], record: &SendRecord) -> Result<TransferRow, String> {
    let sent = record.sent_amount().ok_or_else(|| {
        format!(
            "send journal row {} has recipient amounts that do not sum",
            hex::encode(txid)
        )
    })?;
    let (status, height, confirmed) = match record.state {
        SendState::Dispatched => ("pending", 0, false),
        SendState::Confirmed { height } => ("confirmed", height, true),
        SendState::TerminalRejected => ("failed", 0, false),
        SendState::PresumedDead => ("dropped", 0, false),
    };
    Ok(TransferRow {
        hash: hex::encode(txid),
        amount: sent,
        fee: record.fee,
        height,
        timestamp: 0,
        direction: "out".into(),
        status: status.into(),
        confirmed,
        pqc_protected: true,
    })
}

/// Archival staker status (GUI-PR3).
#[derive(Debug, Clone)]
pub struct StakerStatus {
    pub staking_enabled: bool,
    pub has_stake_engine: bool,
    pub bonded_slot_count: u32,
    pub has_pscan: bool,
}

/// Outcome of `activate_staker` (bond sealed, not yet broadcast).
#[derive(Debug, Clone)]
pub struct ActivateStakerOutcome {
    pub slot: u32,
    pub swept_inputs: usize,
    pub resumed: bool,
    pub state: &'static str,
}

impl From<FirstStakeOutcome> for ActivateStakerOutcome {
    fn from(o: FirstStakeOutcome) -> Self {
        Self {
            slot: o.p_slot,
            swept_inputs: o.swept_inputs,
            resumed: o.resumed,
            state: "pending_dispatch",
        }
    }
}

/// Outcome of a drainable-`P` read (DS-PR-3 PR-B).
///
/// Two-armed, mirroring the core `DrainBalanceReadError` split so the
/// distinction survives to the UI: `Ready` carries the anchored aggregate
/// spendable scalar (atomic units); `Syncing` signals the transient anchor arm
/// (the reference is not yet available — render a placeholder, never a zero). A
/// non-transient fault is *not* an arm here — it stays an `Err(String)` on the
/// read method, so a bad read never masquerades as a value. No `Clone`: no
/// caller needs a second copy (rule 21).
#[derive(Debug)]
pub enum DrainBalanceRead {
    /// Anchored aggregate spendable `P` scalar, atomic units.
    Ready { spendable: u64 },
    /// The send-path reference is not yet anchorable (transient; render
    /// "syncing"). `detail` is static operator text — no amount, no gindex.
    Syncing { detail: String },
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
    // Trim trailing slashes *before* the `/json_rpc` suffix so a base like
    // `http://host:port/json_rpc/` collapses to `http://host:port` rather
    // than leaving a stray `json_rpc` segment (the caller already passes a
    // stripped base, but stay defensive at the daemon-client seam).
    let trimmed = daemon_http_base.trim_end_matches('/');
    let url = trimmed
        .strip_suffix("/json_rpc")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_owned();
    let rpc = HttpRpc::new(url)
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

/// Re-arm P-scan on restore paths; degrade to None rather than failing open.
async fn restart_pscan(shared: &SharedEngine) -> Option<PScanHandle> {
    match Engine::start_pscan_if_staker(shared.clone()).await {
        Ok(handle) => handle,
        Err(e) => {
            warn!(error = %e, "failed to re-arm P-scan while restoring open wallet");
            None
        }
    }
}

fn map_open_err(e: shekyl_engine_core::OpenError) -> String {
    format!("wallet error: {e}")
}

fn map_first_stake_err(e: FirstStakeError) -> String {
    match e {
        FirstStakeError::BondInFlight => {
            "a signed bond post is already awaiting dispatch (stake in flight)".into()
        }
        FirstStakeError::AlreadyStaked => "this wallet is already an active staker".into(),
        FirstStakeError::Funding(detail) => {
            format!(
                "not ready to stake ({detail}); fund the persona (stake_in) and sync, then retry"
            )
        }
        FirstStakeError::FeeEstimate(_) => {
            "fee estimation failed; check the daemon connection and retry".into()
        }
        FirstStakeError::NoStakeEngine => {
            "stake engine not ready after intent open; retry activation".into()
        }
        FirstStakeError::WrongSlot { .. } => format!("stake: {e}"),
        FirstStakeError::State(d) => {
            format!("stake preflight failed ({d}); nothing durable was written")
        }
        FirstStakeError::Persist(d) | FirstStakeError::Engine(d) => {
            format!("stake failed mid-flow ({d}); call activate again to resume")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shekyl_engine_state::SendRecipient;
    use std::collections::BTreeMap;

    fn sample_record(state: SendState, fee: u64, amounts: &[u64]) -> SendRecord {
        SendRecord {
            dispatched_at_height: 10,
            fee,
            recipients: amounts
                .iter()
                .map(|&amount| SendRecipient {
                    address: "SkTestAddr".into(),
                    amount,
                })
                .collect(),
            change_amount: 0,
            inputs: vec![],
            lock_baseline: None,
            state,
        }
    }

    #[test]
    fn engine_wallet_base_appends_wallet_suffix() {
        let p = engine_wallet_base(Path::new("/tmp/wallets"), "Alice");
        assert_eq!(p, PathBuf::from("/tmp/wallets/Alice.wallet"));
    }

    #[test]
    fn outgoing_dispatched_is_pending_with_no_height() {
        let txid = [0xabu8; 32];
        let row = project_outgoing_row(&txid, &sample_record(SendState::Dispatched, 100, &[1_000]))
            .expect("project");
        assert_eq!(row.direction, "out");
        assert_eq!(row.status, "pending");
        assert!(!row.confirmed);
        assert_eq!(row.height, 0);
        assert_eq!(row.amount, 1_000);
        assert_eq!(row.fee, 100);
        assert_eq!(row.hash, hex::encode(txid));
    }

    #[test]
    fn outgoing_confirmed_carries_inclusion_height() {
        let row = project_outgoing_row(
            &[1u8; 32],
            &sample_record(SendState::Confirmed { height: 42 }, 7, &[500, 250]),
        )
        .expect("project");
        assert_eq!(row.status, "confirmed");
        assert!(row.confirmed);
        assert_eq!(row.height, 42);
        assert_eq!(row.amount, 750);
    }

    #[test]
    fn outgoing_failed_and_dropped_never_look_pending_or_confirmed() {
        let failed = project_outgoing_row(
            &[2u8; 32],
            &sample_record(SendState::TerminalRejected, 1, &[9]),
        )
        .expect("failed");
        assert_eq!(failed.status, "failed");
        assert!(!failed.confirmed);
        assert_eq!(failed.height, 0);

        let dropped =
            project_outgoing_row(&[3u8; 32], &sample_record(SendState::PresumedDead, 1, &[9]))
                .expect("dropped");
        assert_eq!(dropped.status, "dropped");
        assert!(!dropped.confirmed);
        assert_eq!(dropped.height, 0);
    }

    #[test]
    fn merge_puts_unsettled_sends_above_mined_rows() {
        let mut journal = BTreeMap::new();
        journal.insert([0x11; 32], sample_record(SendState::Dispatched, 1, &[100]));
        journal.insert(
            [0x22; 32],
            sample_record(SendState::Confirmed { height: 5 }, 1, &[200]),
        );

        let rows = merge_transfer_history(&[], &journal, 10).expect("merge");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].status, "pending");
        assert_eq!(rows[0].height, 0);
        assert_eq!(rows[1].status, "confirmed");
        assert_eq!(rows[1].height, 5);
    }
}
