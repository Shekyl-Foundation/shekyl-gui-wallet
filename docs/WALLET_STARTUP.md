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
in-process through `wallet_bridge.rs`, which combines two components:

1. **`shekyl-engine-rpc` (Rust crate)** -- an FFI wrapper around C++ `wallet2`.
   Today this provides wallet creation, opening, key management, and the
   construction half of transactions. Despite the crate name, it is currently
   linked directly into the GUI process; it does not run as a separate RPC
   server.
2. **`shekyl-scanner` (Rust crate)** -- pure-Rust output scanning and balance
   tracking. Runs in a background tokio task that polls the daemon over HTTP
   and updates `WalletState` as new blocks arrive.

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
│  │  wallet_bridge.rs              │  │
│  │  ┌──────────────┐ ┌──────────┐ │  │
│  │  │ Wallet2 FFI  │ │ scanner  │ │  │
│  │  │ (C++ wallet2)│ │ (Rust)   │ │  │
│  │  └──────────────┘ └──────────┘ │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

### A note on direction

The C++ FFI bridge is **transitional**. The long-term direction is a pure-Rust
path through `shekyl-engine-rpc` (which will eventually replace `wallet2`
entirely), with FFI retained only in the few places where C++ provides
specific, hardened, audited value. The "RPC" in the crate name reflects that
end state, not the current in-process linkage.

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
refreshes the wallet-file list. The override currently lives in
`AppState` only and does not persist across launches -- see
`docs/FOLLOWUPS.md` for the persistence follow-up.

### Filename normalization

When the user types a wallet name like `My Wallet`, `wallet_name::sanitize`
normalizes it to `My_Wallet` before any filesystem call, and
`wallet_name::build_wallet_path` joins it with the active directory via
`PathBuf::join` so the host separator is always correct (e.g.
`C:\Users\<user>\AppData\Roaming\shekyl\wallets\My_Wallet.keys` on
Windows). Opening a wallet uses dual-search: the sanitized name is tried
first, and the raw name is tried as a fallback for wallets created on
pre-normalization builds.

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

1. `wallet2` (C++) opens the `.keys` file and unlocks it with the supplied
   password.
2. The bridge extracts scanner keys (view key pair, spend public key) from
   the C++ wallet via FFI.
3. A background tokio task is spawned that runs `shekyl_scanner::Scanner`
   against the daemon, polling for new blocks and updating
   `Arc<TokioMutex<WalletState>>`.
4. A `CancellationToken` is stored so the sync loop can be stopped on close
   or shutdown.

When a wallet is closed (`close_wallet`) or the window is destroyed:

1. The cancellation token is fired; the sync loop finishes its current
   block and exits.
2. The C++ `wallet2` instance is dropped; secrets are wiped via `Zeroize`.
3. Scanner state is cleared.

### Concurrency Model

- The C++ FFI handle (`Wallet2`) sits behind a `std::sync::Mutex` for
  synchronous access from Tauri commands.
- Scanner state sits behind a `tokio::sync::Mutex` so the background sync
  loop and Tauri commands can both read it without blocking the async
  runtime.
- All blocking C++ FFI calls from async contexts go through
  `tokio::task::spawn_blocking` to avoid stalling the async executor.

---

## Create Wallet Flow

1. Frontend calls `create_wallet(name, password, language)`.
2. Bridge calls `wallet2` FFI to create the wallet, then queries the
   mnemonic and primary address.
3. Returns `CreateWalletResult` with name, address, seed, language, network.
4. Frontend displays seed in a numbered 5x5 grid.
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

### From Seed Phrase

Calls `restore_deterministic_wallet(filename, seed, password, language, restore_height)`.
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
