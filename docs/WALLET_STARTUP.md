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
were only ever backed by the old path are absent from the default build
rather than registered refusals (rule 28, stub leg): import-from-keys and
scanner freeze/thaw are deleted; PQC multisig compiles only under
`--features multisig`.

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
| `importing`    | Import Wallet       | Restoring from the recovery phrase         |
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

`ensure_wallet_dir` (Tauri command) guarantees the configured wallet
directory exists before any create/open flow runs. Nothing else is
initialised at startup: the Engine connects to the daemon per wallet-open,
and no external process is started.

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

`create_wallet(name, password)` creates the wallet through the Engine and
returns the contract's `CreateWalletResult` — `wallet` (the `WalletHandle`:
name, capability `FULL`, network, and `restore_height_hint` only when the
contract reports one) and the backup exactly once, in the network's
encoding: `mnemonic` (24 words) on mainnet/stagenet or `raw_seed_hex` on
testnet. The address is not on the result; the page reads it from
`get_primary_address` once the wallet is open, and a failed read is shown
rather than a blank address. There is no `seed_language` and no
mnemonic-language argument: which backup field is present says which
encoding the Engine chose.

1. **setup** — name, password and confirmation.
2. **seed** — the phrase in a numbered grid, or the testnet hex seed on its
   own. "Copy to clipboard" hands that backup to Rust, which owns the
   clipboard's timed, hash-checked clear (`GUI_SECURITY.md` "Recovery phrase").
3. **confirm** — four randomly chosen words, or a re-entry of the hex seed.
4. **done** — transitions to `phase: "ready"`.

Every new wallet is a v3 wallet. The Engine derives hybrid Ed25519 + ML-DSA-65
spend authorization as part of creation; there is no flag that turns it off.

New wallets also generate ML-KEM-768 key material for the Bech32m address
format (`shekyl1:<version><classical ~103 chars>/<pqc ~1750 chars>`, ~1,870
characters total), enabling per-output PQC key derivation via hybrid KEM
(X25519 + ML-KEM-768) when receiving transactions. This prevents transaction
linkability even against quantum adversaries. The wallet displays the
classical segment by default; the PQC segment is handled internally.

---

## Import Wallet Flows

### From Recovery Phrase

The only restore path. `restore_wallet(name, password, mnemonic,
restore_height)` — the contract's method and parameters — validates the backup in the encoding the running network
hands out at creation (`validate_seed_backup`: a 24-word phrase on
mainnet/stagenet, the 32-byte raw seed as 64 hex characters on testnet —
the contract's `restore_wallet`) and calls `EngineSession::restore_from_backup`,
whose `master_seed_from_backup` is the inverse of the create path's
`generate_seed_material`. The hybrid post-quantum keys are derived from the
seed, so nothing is "generated for" a restored wallet and no passphrase or
mnemonic language is taken. `restore_height` defaults to 0 (full scan). On
success the page shows "Restore complete" and transitions to `phase: "ready"`.

There is no import from raw spend/view keys: that was a Wallet2 path whose
GUI command had become an unconditional refusal, and the command-surface
gate (`scripts/ci/check_command_surface.sh`, stub leg) now refuses such a
command. The contract's `restore_wallet` takes a mnemonic only.

---

## Transfer Flow

Outgoing transactions follow the wallet contract's own three steps in
`src-tauri/src/send.rs` — `get_default_fee_priority` → `build_pending_tx`
→ `submit_pending_tx` / `discard_pending_tx` — entirely in Rust through the
Engine. See `GUI_SECURITY.md` "Send Flow" for the invariant (one built
transaction per user intent; the fee the user confirms is the fee that
ships) and the reservation-ownership rules.

---

## Daemon Connection

The wallet connects to a `shekyld` daemon over HTTP. Default ports:

| Network   | Daemon RPC |
|-----------|------------|
| Mainnet   | 11029      |
| Testnet   | 12029      |
| Stagenet  | 13029      |

The Engine (transaction construction and submission) and the scanner (block
fetching) both talk to that same daemon endpoint.

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
