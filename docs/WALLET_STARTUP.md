# Wallet Startup Flow Design

This document describes the wallet startup flow -- how the GUI wallet detects
existing wallets, authenticates the user, and ensures they are running with a
legitimate v3 (PQC-enabled) wallet before accessing the main application.

---

## Architecture Overview

The GUI wallet manages two external processes:

1. **shekyld** (daemon) -- blockchain node, expected to be running
   independently or managed by the wallet in the future.
2. **shekyl-wallet-rpc** -- wallet JSON-RPC server, spawned and managed by
   the GUI wallet as a child process.

Communication with both follows the same pattern: JSON-RPC over HTTP to
localhost.

```
┌──────────────┐     JSON-RPC      ┌────────────────────┐     JSON-RPC     ┌──────────┐
│  Tauri App   │ ───────────────── │  shekyl-wallet-rpc │ ─────────────── │  shekyld  │
│  (Rust + UI) │ :11030/json_rpc  │  (child process)   │ :11029/json_rpc │  (daemon) │
└──────────────┘                   └────────────────────┘                  └──────────┘
```

The wallet-rpc runs in `--wallet-dir` mode, which allows creating, opening,
and closing wallets via RPC without restarting the process.

---

## State Machine

The frontend uses a phase-based state machine to control what the user sees:

| Phase          | Screen              | Description                                |
|----------------|---------------------|--------------------------------------------|
| `loading`      | Loading screen      | Spawning wallet-rpc, scanning for wallets  |
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

On startup, the Tauri backend scans the platform default wallet directory for
`.keys` files:

| Platform | Default Path                                    |
|----------|-------------------------------------------------|
| Linux    | `~/.shekyl/wallets/`                            |
| macOS    | `~/Library/Application Support/shekyl/wallets/` |
| Windows  | `%APPDATA%\shekyl\wallets\`                     |

Detection is a pure filesystem operation -- no RPC needed. The `check_wallet_files`
Tauri command returns a list of `WalletFileInfo` structs (name, path, modified
timestamp) sorted by most recently modified.

---

## Wallet-RPC Process Management

### Lifecycle

1. **Spawn**: `init_wallet_rpc` Tauri command spawns the binary with:
   ```
   shekyl-wallet-rpc --wallet-dir=<dir> --rpc-bind-port=<port>
       --daemon-address=<daemon> --disable-rpc-login --non-interactive
   ```

2. **Readiness**: Polls `get_version` RPC every 300ms with a 15s timeout.

3. **Stale process detection**: Before spawning, checks if the port is already
   in use (another instance, or unclean shutdown). If occupied and responding,
   reuses the existing process.

4. **Shutdown**: On window close or explicit `shutdown_wallet_rpc`:
   - Send `stop_wallet` RPC for graceful exit.
   - Wait 1-2 seconds.
   - If still running, SIGTERM then SIGKILL.

### Binary Resolution

Layered strategy (first match wins):

1. **Tauri sidecar** (production) -- `externalBin` in `tauri.conf.json`.
2. **PATH lookup** (development) -- `which shekyl-wallet-rpc`.
3. **Platform-specific install dirs** -- `/usr/local/bin`, homebrew, Program Files.
4. **User-configured path** (Settings override).

---

## Create Wallet Flow

1. Frontend calls `create_wallet(name, password, language)`.
2. Backend calls wallet-rpc `create_wallet` then `query_key("mnemonic")` and
   `get_address(0)`.
3. Returns `CreateWalletResult` with name, address, seed, language, network.
4. Frontend displays seed in a numbered 5x5 grid.
5. Frontend challenges user to enter 4 randomly chosen words.
6. On success, transitions to `phase: "ready"`.

The wallet created by `shekyl-wallet-rpc` automatically includes PQC key
material (Ed25519 + ML-DSA-65) since the wallet-rpc links against shekyl-core
which calls `generate_pqc_key_material()` during account generation. No
special flags needed -- all new wallets are v3 PQC wallets.

---

## Import Wallet Flows

### From Seed Phrase

Calls `restore_deterministic_wallet(filename, seed, password, language, restore_height)`.
PQC keys are generated automatically for restored wallets via
`generate_pqc_for_restored_address()` in wallet2.

### From Keys

Calls `generate_from_keys(filename, address, spendkey, viewkey, password, language, restore_height)`.
If the address includes PQC public key bytes, they are preserved. If not,
wallet2 generates fresh PQC key material on the restore path.

Both flows set `restore_height` (default 0 = full scan) and transition to
`phase: "ready"` on success.

---

## Sidecar Bundling (Future)

When compiled daemon and wallet-rpc binaries are ready for release packaging:

1. Place platform-specific binaries in `src-tauri/binaries/` with Tauri's
   target-triple naming convention.
2. Add `"externalBin": ["binaries/shekyld", "binaries/shekyl-wallet-rpc"]`
   to `tauri.conf.json` under `bundle`.
3. Tauri's bundler (`deb`, `msi`, `dmg`, `AppImage`) automatically packages
   them into the installer.

See `src-tauri/binaries/README.md` for naming conventions.

---

## Port Assignments

| Network   | Daemon RPC | Wallet RPC |
|-----------|------------|------------|
| Mainnet   | 11029      | 11030      |
| Testnet   | 12029      | 12030      |
| Stagenet  | 13029      | 13030      |

---

## Error Scenarios

| Error                         | User Experience                                      |
|-------------------------------|------------------------------------------------------|
| wallet-rpc binary not found   | Error on loading screen with install instructions    |
| Wrong password                | Inline error on Unlock, password field stays focused |
| Daemon not connected          | Wallet opens normally; "Daemon offline" banner shows |
| Seed confirmation wrong       | User re-attempts; wallet not regenerated             |
| Stale process on port         | Reuses existing wallet-rpc if healthy                |
| App crash / unclean shutdown  | Next launch detects occupied port, reuses or kills   |
