# Wallet Startup Flow Design

This document describes the wallet startup flow -- how the GUI wallet detects
existing wallets, authenticates the user, and ensures they are running with a
legitimate v3 (PQC-enabled) wallet before accessing the main application.

v3 means hybrid Ed25519 + ML-DSA-65 spend authorization (`pqc_auth`, one proof
per input being spent) plus FCMP++ membership proofs for transaction privacy.
CLSAG ring signatures are never used -- Shekyl uses FCMP++ from genesis,
providing full-UTXO-set anonymity via curve trees.

---

## Architecture Overview

The GUI wallet is a single Tauri process. Wallet operations are performed
in-process through `engine_session.rs`, which combines two components:

1. **`shekyl-engine-core` (Rust crate)** -- the pure-Rust `Engine`, embedded
   directly. It provides wallet creation, opening, key management, and
   transaction construction. There is no C++ `wallet2` and no separate wallet
   process. (This replaced the transitional `wallet_bridge.rs` / `Wallet2` FFI
   path at GUI-PR1; the `shekyl-engine-rpc` crate that path went through has
   since been deleted from `shekyl-core` outright.)
2. **`shekyl-scanner` (Rust crate)** -- pure-Rust output scanning and balance
   tracking. Runs in a background tokio task that polls the daemon over HTTP
   and updates a `(LedgerBlock, LedgerIndexes)` pair as new blocks arrive.
   `LedgerBlock` holds the wallet's view of confirmed outputs
   and per-output state, and `LedgerIndexes` holds the spend/freeze indexes.
   Block ingestion goes through
   `LedgerIndexes::process_scanned_outputs(&mut ledger_block, height,
   block_hash, outputs)` from the `LedgerIndexesExt` trait; reorg handling
   uses `LedgerIndexes::handle_reorg`. Both sides live behind one
   `tokio::sync::Mutex` so the in-process sync loop and Tauri commands see
   a consistent snapshot.

```
┌──────────────────────────────────────┐                ┌──────────┐
│  Tauri App (single process)          │   HTTP/JSON-RPC │          │
│                                      │ ──────────────► │  shekyld │
│  ┌────────────────────────────────┐  │                 │ (daemon) │
│  │  React UI (webview)            │  │ ◄────────────── │          │
│  └────────────┬───────────────────┘  │                └──────────┘
│               │ Tauri IPC            │
│  ┌────────────▼───────────────────┐  │
│  │  commands.rs                   │  │
│  └────────────┬───────────────────┘  │
│  ┌────────────▼───────────────────┐  │
│  │  engine_session.rs             │  │
│  │  ┌──────────────┐ ┌──────────┐ │  │
│  │  │ Engine       │ │ scanner  │ │  │
│  │  │ (pure Rust)  │ │ (Rust)   │ │  │
│  │  └──────────────┘ └──────────┘ │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

### A note on direction

The pure-Rust path is **the current path, not a target**. The transitional
C++ `wallet2` FFI bridge is gone: `wallet_bridge.rs` was deleted at GUI-PR1,
the `shekyl-ffi` / `shekyl-engine-rpc` deps and the C++ static linkage went
with it, and `shekyl-engine-rpc` itself has since been deleted from
`shekyl-core`. Nothing in this process links C++ wallet code. Features that
were only ever backed by the old path (import-from-keys, PQC multisig,
scanner freeze/thaw) return honest "not available on the Engine backend"
errors until they are ported.

---

## State Machine

The frontend uses a phase-based state machine to control what the user sees:

| Phase          | Screen              | Description                                |
|----------------|---------------------|--------------------------------------------|
| `loading`      | Loading screen      | Initializing wallet bridge, scanning files |
| `no_wallet`    | Welcome             | No .keys files found; offer create/import  |
| `select_wallet`| Unlock (with picker)| Multiple .keys files; user picks one       |
| `unlock`       | Unlock              | Single .keys file; enter password          |
| `creating`     | Create Wallet       | In the middle of wallet creation wizard    |
| `importing`    | Import Wallet       | Restoring from seed/keys                   |
| `ready`        | Main app (Dashboard)| Wallet is open and authenticated           |

Transitions:

```
loading ──┬──► no_wallet ──┬──► creating ──► ready
          │                └──► importing ──► ready
          ├──► unlock ─────────────────────► ready
          └──► select_wallet ──► unlock ──► ready

ready ──► unlock (lock wallet)
ready ──► no_wallet (close wallet, no other wallets exist)
```

---

## Wallet File Detection

On startup, the Tauri backend ensures the active wallet directory exists
(via `wallet_name::ensure_dir_exists`, which maps to
`std::fs::create_dir_all` -- equivalent to `mkdir -p`) and then scans it
for `.keys` files.

### Default directory

| Platform | Default Path                                    |
|----------|-------------------------------------------------|
| Linux    | `~/.shekyl/wallets/`                            |
| macOS    | `~/Library/Application Support/shekyl/wallets/` |
| Windows  | `%APPDATA%\shekyl\wallets\`                     |

### Custom directory

Users can override the default via the "Advanced: wallet file location"
disclosure on the Create, Import, and Unlock screens. The Tauri commands
`set_wallet_dir(dir)`, `reset_wallet_dir()`, and `get_wallet_dir()` back
this UI; `set_wallet_dir` validates the path, runs `mkdir -p` on it, and
refreshes the wallet-file list.

The override persists across launches via `gui-config.json` in the
Tauri app-config dir (`~/.config/org.shekyl.wallet/gui-config.json` on
Linux, `~/Library/Application Support/org.shekyl.wallet/gui-config.json`
on macOS, `%APPDATA%\org.shekyl.wallet\gui-config.json` on Windows;
see `src-tauri/src/gui_config.rs`). At startup, `AppState::new` reads
the override and probes the directory. If the override is missing,
malformed, or unreachable (permission denied, target-is-a-file, broken
symlink), the app silently falls back to the platform default; the
original path is surfaced via `get_wallet_dir`'s `fallback_from` field
so the Advanced disclosure can render a soft warning banner. Explicit
`set_wallet_dir` / `reset_wallet_dir` calls write the new state and
clear `fallback_from`. Writes are atomic (`*.tmp` + rename),
best-effort, and logged at `warn!` on failure.

### Filename normalization

When the user types a wallet name like `My Wallet`, `wallet_name::sanitize`
normalizes it to `My_Wallet` before any filesystem call, and
`wallet_name::build_wallet_path` joins it with the active directory via
`PathBuf::join` so the host separator is always correct (e.g.
`C:\Users\<user>\AppData\Roaming\shekyl\wallets\My_Wallet.keys` on
Windows).

As of alpha.5, `sanitize` is the single source of truth for filename
policy. Any character outside `[A-Za-z0-9_\-.]` plus the Unicode-letter
superset is replaced with `_`; runs of `_` collapse to a single
underscore; leading/trailing `_`, `.`, and whitespace are trimmed. This
covers path separators (`/`, `\`), Windows-reserved characters (`<>:"|?*`),
null bytes, control characters, and emoji uniformly. `validate_wallet_name`
only checks non-empty and length-under-cap after sanitization runs.

Opening a wallet still uses dual-search: the sanitized name is tried
first, and the raw name is tried as a fallback for wallets created on
pre-normalization builds. The fallback is scheduled for removal in
alpha.6 once the alpha.5 sanitize-broadening notice and helper text
have shipped (see `docs/FOLLOWUPS.md`).

Detection itself is a pure filesystem operation -- no FFI or daemon
connection needed. The `check_wallet_files` Tauri command returns a list
of `WalletFileInfo` structs (name, path, modified timestamp) sorted by
most recently modified.

---

## Wallet Bridge Lifecycle

### Initialization

`init_wallet_rpc` (Tauri command -- name retained for IPC compatibility)
initializes the wallet bridge with the network type, daemon address, and
wallet directory. No external process is started; this is a synchronous
in-process FFI initialization.

### Open / Close

When a wallet is opened (`open_wallet`):

1. `Engine::open` (pure Rust) opens the `{name}.wallet` / `{name}.wallet.keys`
   envelope pair and unlocks it with the supplied password.
2. The Engine owns the scanner keys internally — nothing is extracted across
   an FFI boundary, and no secret crosses into GUI-owned state (rule 36).
3. If the wallet is a staker, `Engine::start_pscan_if_staker` starts the
   `P`-scan task. Chain scanning is driven by `Engine::start_refresh`, which
   polls the daemon, runs blocks through `shekyl_scanner::Scanner::scan`, and
   applies the results — including spend detection and reorg handling —
   inside the Engine. The GUI does not own a sync loop.
4. The returned handles (`PScanHandle`, refresh handle) are held by the
   session so they can be shut down on close.

When a wallet is closed (`close_wallet`) or the window is destroyed:

1. The scan handles are shut down; in-flight work drains.
2. The `Engine` is dropped; secrets are wiped via `Zeroize`.
3. Session state is cleared.

### Concurrency Model

- The `Engine` is held as a `SharedEngine` (an `Arc`-wrapped handle); Tauri
  commands clone it rather than holding a lock across await points.
- Scan-derived state lives inside the Engine, so background scanning and
  Tauri command reads do not contend on a GUI-owned mutex.
- There are no blocking FFI calls to shield the async executor from — the
  wallet path is pure Rust and async end to end.

---

## Create Wallet Flow

**Current (pre–BIP-39 integration):** Frontend calls
`create_wallet(name, password, language)`. The `language` parameter is legacy
and will be removed in the integration PR.

**Planned (after shekyl-core BIP-39 FFI + gui integration PR):**
`create_wallet(name, password)` → `wallet2_ffi_create_wallet_from_bip39`, then
`query_key("mnemonic")` for the 24-word recovery phrase. No seed-language
parameter.

1. Bridge creates the wallet file and queries the recovery phrase and primary
   address.
2. Returns `CreateWalletResult` with name, address, seed, network.
3. Frontend displays the phrase in a numbered grid (24 words).
5. Frontend challenges user to enter 4 randomly chosen words.
6. On success, transitions to `phase: "ready"`.

The wallet automatically includes PQC key material (Ed25519 + ML-DSA-65)
because `wallet2` calls `generate_pqc_key_material()` during account
generation. No special flags needed -- all new wallets are v3 PQC wallets.

New wallets also generate ML-KEM-768 key material for the Bech32m address
format (`shekyl1:<version><classical ~103 chars>/<pqc ~1750 chars>`, ~1,870
characters total), enabling per-output PQC key derivation via hybrid KEM
(X25519 + ML-KEM-768) when receiving transactions. This prevents transaction
linkability even against quantum adversaries. The wallet displays the
classical segment by default; the PQC segment is handled internally.

---

## Import Wallet Flows

### From Recovery Phrase

**Current (pre–BIP-39 integration):** Calls
`restore_deterministic_wallet(filename, seed, password, language, restore_height)`.
The GUI prep PR validates 24-word input client-side; full BIP-39 restore
requires the integration PR and updated shekyl-core FFI.

**Planned:** `restore_from_bip39(filename, phrase, password, passphrase, restore_height)`
(replaces Electrum restore). Optional BIP-39 passphrase maps to
`seed_passphrase` in wallet2 JSON semantics per
`shekyl-core/docs/design/ELECTRUM_WORDS_REMOVAL.md` §4.5.1.

PQC keys are generated automatically for restored wallets via
`generate_pqc_for_restored_address()` in `wallet2`.

### From Keys

Calls `generate_from_keys(filename, address, spendkey, viewkey, password, language, restore_height)`.
If the address includes PQC public key bytes, they are preserved. If not,
`wallet2` generates fresh PQC key material on the restore path.

Both flows set `restore_height` (default 0 = full scan) and transition to
`phase: "ready"` on success.

---

## Transfer Flow (Native-Sign)

Outgoing transactions use the native-sign path:

1. **C++ prepare** -- `wallet2` selects inputs, computes change, and builds
   the transaction skeleton (output construction, commitment masks; no ring
   selection -- FCMP++ replaces ring signatures).
2. **Rust sign** -- the FCMP++ membership proof and PQC `pqc_auth` blobs
   are produced by the Rust signing crates.
3. **C++ finalize** -- `wallet2` records the transaction, marks inputs
   spent, and submits to the daemon.

If finalize fails after sign, the bridge returns an error to the frontend;
inputs remain spendable from the wallet's perspective and will be
reconsidered on the next transfer attempt.

---

## Daemon Connection

The wallet connects to a `shekyld` daemon over HTTP. Default ports:

| Network   | Daemon RPC |
|-----------|------------|
| Mainnet   | 11029      |
| Testnet   | 12029      |
| Stagenet  | 13029      |

Both the C++ `wallet2` instance (for transaction submission, key image
checks) and the Rust scanner (for block fetching) talk to the same daemon
endpoint.

---

## Error Scenarios

| Error                         | User Experience                                      |
|-------------------------------|------------------------------------------------------|
| Wrong password                | Inline error on Unlock, password field stays focused |
| Daemon not connected          | Wallet opens normally; "Daemon offline" banner shows |
| Seed confirmation wrong       | User re-attempts; wallet not regenerated             |
| Sync loop fails to start      | Wallet opens; scanner inactive; banner warns         |
| Transfer finalize fails       | Error surfaced; inputs remain spendable for retry    |
| App crash / unclean shutdown  | Next launch re-opens normally; wallet file intact    |
