# GUI Wallet Security Model

This document describes the security architecture of the Shekyl GUI wallet (Tauri v2 desktop application), its threat model, and known limitations.

## Architecture Overview

```
┌──────────────────────┐
│   React UI (Webview)  │  ← CSP-restricted, no remote scripts
│                      │
│   Tauri IPC bridge   │  ← validate.rs: all inputs validated
├──────────────────────┤
│   wallet_bridge.rs   │  ← Rust: type-safe, no unsafe
│   commands.rs        │
├──────────────────────┤
│ shekyl-engine-rpc    │  ← C++ wallet2 FFI (prepare/finalize)
│ shekyl-scanner       │  ← Rust scanner (scan/balance/state)
│ shekyl-tx-builder    │  ← Rust signing (native-sign)
└──────────────────────┘
```

The React webview communicates with the Rust backend exclusively through Tauri's IPC mechanism. No network access is permitted from the webview.

## Content Security Policy

The CSP is set in `tauri.conf.json`:

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
img-src 'self' data:; font-src 'self' data:; connect-src ipc: http://ipc.localhost
```

This prevents:
- Loading remote scripts (XSS via CDN compromise)
- Fetching external resources (data exfiltration)
- Opening external URLs without user action
- Inline scripts (only `'self'` scripts allowed)

`'unsafe-inline'` for `style-src` is required by Tailwind CSS. This is an acceptable trade-off: inline styles cannot execute code.

## Tauri Capabilities

Capabilities are defined in `capabilities/default.json`:
- Scoped to `"windows": ["main"]` only
- Permissions: `core:default`, `opener:default`
- Sensitive commands (`get_seed`, `transfer`, `import_wallet_from_seed`, `import_wallet_from_keys`, `query_key`) are only callable from the main window context

## Input Validation

Every Tauri command that accepts user input validates before reaching the C++ FFI or Rust scanner. The `validate.rs` module enforces:

| Input | Validation |
|-------|-----------|
| Address | Bech32m decode via `shekyl-address` crate |
| Amount | Non-zero u64 |
| Wallet name | No path separators, no dots prefix, max 255 chars |
| Password | No null bytes, max 1024 chars |
| Seed phrase | ASCII, 1-30 words, no null bytes |
| Secret keys | Exact 64 hex chars |
| Key images | Exact 64 hex chars |
| Staking tier | 0, 1, or 2 |

Malformed inputs are rejected at the Rust bridge layer with a human-readable error. No malformed data reaches C++.

## Transfer Flow Security

The transfer uses a three-phase native-sign path:

1. **C++ Prepare**: wallet2 selects UTXOs, builds tx prefix, returns structured JSON
2. **Rust Sign**: `shekyl-tx-builder` generates FCMP++ proof, BP+ proof, PQC auth
3. **C++ Finalize**: wallet2 inserts proofs, broadcasts to daemon

No optimistic spent-marking is performed on the scanner side. The scanner's sync loop is the sole authority for marking outputs as spent — it does so only when key images appear on-chain. If signing succeeds but finalize fails (daemon unreachable, relay rejected), `WalletState` is unaffected and no rollback is needed. Outputs remain spendable for a retry.

## Secret Key Handling

- All wallet secrets (spend key, view key, X25519 SK, ML-KEM DK) are wrapped in `Zeroizing<T>` in Rust and wiped on drop
- The scanner keys extracted via `wallet2_ffi_get_scanner_keys` are zeroized immediately after constructing the `ViewPair` and `Scanner`
- `WalletState` implements `Drop` with `zeroize()` on all sensitive fields
- `TransferDetails` implements `Drop` with `zeroize()` and a redacting `Debug`
- On `close_wallet`, the sync loop is cancelled and the scanner state is replaced (triggering Drop/zeroize on the old state)
- On `shutdown` (window destroy), the same wipe occurs — sync loop cancelled and scanner state replaced

## Seed/Password Entry Threat Model

### Known OS-Level Exposures

These are inherent to any desktop wallet with a GUI:

1. **Keyboard pipeline**: Key events pass through the OS event pipeline (accessibility APIs, input method editors, predictive text engines). The CLI's `getpass`-style prompt bypasses most of this.

2. **Clipboard**: Pasting a seed means it hits the OS clipboard. Clipboard managers and malware can log clipboard contents.

3. **Webview profile isolation**: Tauri uses an isolated webview profile (not shared with the system browser). Browser extensions cannot access the wallet's DOM.

4. **Screen capture**: The seed display page is visible to screen capture tools and remote desktop software.

### Mitigations in Place

- CSP prevents JavaScript injection that could scrape DOM contents
- No seed/password values are logged, serialized to plaintext, or included in error messages
- The webview profile is isolated from the system browser

### User Guidance

- Use a dedicated, clean machine for seed entry when possible
- Clear clipboard after pasting seed material
- Avoid screen-sharing or remote desktop during seed display
- Store the seed offline (paper/metal backup), not in digital form

## Future Hardening Roadmap

These are tracked for implementation in future releases:

- [ ] **On-screen keyboard for seed entry** — bypasses OS keyboard pipeline, accessibility loggers, predictive text
- [ ] **Seed display with dismissal gesture** — show words once, require explicit acknowledgement, then clear from DOM
- [ ] **Clipboard access denial for seed fields** — prevent clipboard logger exfiltration via `navigator.clipboard` API restriction
- [ ] **Automatic seed field clearing** — if user navigates away, clear seed fields after a short timeout
- [ ] **Memory-locked allocations** — `mlock()` on pages holding wallet secrets in the Rust process
- [ ] **`prctl(PR_SET_DUMPABLE, 0)`** — suppress core dumps containing secrets on Linux

## Scanner State Persistence

The scanner state is currently in-memory only. No partial state is persisted to disk between sessions. On wallet reopen, the scanner re-scans from the wallet's last-known height. The `on_flush` callback in the sync loop is a deliberate no-op pending `WalletState` serde support. This means:

- A crash mid-scan loses in-memory outputs discovered since the last open
- Recovery is automatic: the scanner detects missed blocks on next open and re-scans
- There is no risk of corrupt on-disk state

Persistence (atomic snapshot every N blocks) is tracked in the Future Hardening Roadmap.

## Supply Chain

See the supply chain hardening section in the Phase 4f implementation:
- `package-lock.json` pins all npm dependencies
- `npm audit --audit-level=high` runs in CI
- Node.js version pinned via `.nvmrc`
- Rust toolchain pinned via `rust-toolchain.toml`
