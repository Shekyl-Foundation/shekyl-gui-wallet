# GUI Wallet Security Model

This document describes the security architecture of the Shekyl GUI wallet (Tauri v2 desktop application), its threat model, and known limitations.

## Architecture Overview

```
┌──────────────────────┐
│   React UI (Webview)  │  ← CSP-restricted, no remote scripts
│                      │
│   Tauri IPC bridge   │  ← validate.rs: all inputs validated
├──────────────────────┤
│   engine_session.rs  │  ← Rust: type-safe, no unsafe
│   commands.rs        │
├──────────────────────┤
│ shekyl-engine-core   │  ← Rust Engine (lifecycle/build/submit)
│ shekyl-scanner       │  ← Rust scanner (scan/balance/state)
│ shekyl-tx-builder    │  ← Rust signing (FCMP++ / PQC)
└──────────────────────┘
```

The React webview communicates with the Rust backend exclusively through Tauri's IPC mechanism. No network access is permitted from the webview.

## Daemon identity

The Engine's daemon client is constructed with
`DaemonClient::verifying` (`engine_daemon.rs` `make_daemon`), matching
`shekyl-wallet-rpc`. `make_daemon` runs the four-axis handshake before
it returns, so create/restore refuse a foreign node **before**
`Engine::create` writes a file (the recovery phrase is create-once: it is
returned in the `create_wallet` result and nothing retains a copy — there
is no seed-returning command). Open still fail-closes the session
if a later Engine RPC sees a mismatch. Status-panel polls that still
go through `daemon_rpc.rs` do not run this check — they are a separate
HTTP client.

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
- Sensitive commands (`build_pending_tx` / `submit_pending_tx`, `restore_wallet`, `stake`, `create_payment_request`, `copy_to_clipboard`) are only callable from the main window context
- A pasted `shekyl:` payment link is counterparty-controlled text: `parse_uri` runs in Rust, the Send page only prefills the address and amount from it and shows the label as text, and nothing from the link is trusted, stored or sent
- The command surface itself is gated: `scripts/ci/check_command_surface.sh` holds `generate_handler![...]` to the wallet contract (every command is a contract adapter or declared in `scripts/ci/command_surface.conf`), to its callers (no registered command without a page that invokes it, no invoke without a command), and to honesty (no registered command whose body is an unconditional refusal)

## Input Validation

Commands that take user input check it in `validate.rs` before it reaches the Engine:

| Input | Validation |
|-------|-----------|
| Address | Bech32m decode via `shekyl-address` crate |
| Amount | Non-zero u64, carried across the Tauri edge as a decimal string (`AtomicUnitsString`, `src-tauri/src/wire.rs`) — never a JS `number` |
| Wallet name | No path separators, no dots prefix, max 255 chars |
| Password | No null bytes, max 1024 chars |
| Recovery phrase | Exactly 24 ASCII words, no null bytes |

Malformed inputs are rejected at the Rust bridge with a human-readable error. No malformed input reaches the Engine.

## Send Flow

The GUI drives the wallet engine's own reservation lifecycle under the contract's names (`src-tauri/src/send.rs`): `get_default_fee_priority` → `build_pending_tx` → `submit_pending_tx` / `discard_pending_tx`. The wallet2 prepare/finalize path this section used to describe no longer exists.

- **Nothing is built while the user types.** The fee shown in the form is the daemon's tier quote for the canonical 2-in/2-out shape (weight × rate), fetched once per page. The previous page ran a full FCMP++ build — selection, proving, signing, reservation — on every 500 ms typing pause and discarded it.
- **One built transaction per intent.** Review builds once and shows the exact fee; Confirm submits that reservation with the `content_gen` it was reviewed at. The engine refuses a stale generation, so the user can never broadcast content they did not see.
- **A content change is never resubmitted silently.** If the realized fee or change moved on re-anchor, the reservation is discarded and rebuilt, and the user re-confirms figures they can read.
- **Retained reservations are never discarded by the page.** An ambiguous or still-pending submit may already be on the network; the engine keeps the reservation so a retry cannot double-spend, and the page leaves it alone.
- **Cancel, leaving the page, or closing the window discards** the reservation and releases the funds.
- **Every atomic amount crosses the Tauri edge as a decimal string** (`AtomicUnitsString`), parsed with `BigInt` and shown on the review card at full 9-decimal precision. A JS `number` is lossy above 2^53; a fee rounded at the edge would break the invariant that the fee the user confirms is the fee that ships, and two reservations one atomic unit apart must never display alike.
- **A failed discard keeps the reservation owned.** The page releases ownership only when a submit succeeds, a discard succeeds, or the engine reports it has retained the reservation; a discard that fails is shown, review stays, and a rebuild is never stacked on a reservation that is still live.

Spent-marking is unchanged: the engine's refresh is the sole settlement authority, and a submit verdict is display metadata only.

## Secret Key Handling

- All wallet secrets (spend key, view key, X25519 SK, ML-KEM DK) are wrapped in `Zeroizing<T>` in Rust and wiped on drop
- The scanner keys extracted via `wallet2_ffi_get_scanner_keys` are zeroized immediately after constructing the `ViewPair` and `Scanner`
- `LedgerBlock` / `LedgerIndexes` (the scanner state pair) implement `Drop` with `zeroize()` on all sensitive fields in `shekyl-engine-state`
- `TransferDetails` implements `Drop` with `zeroize()` and a redacting `Debug`
- On `close_wallet`, the sync loop is cancelled and the scanner state is replaced with `(LedgerBlock::empty(), LedgerIndexes::empty())` (triggering Drop/zeroize on the old state)
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
- Clear clipboard after pasting seed material on import; on create the
  wallet clears a phrase it placed (60 s after "Copy", on leaving the page,
  or when the window closes)
- Avoid screen-sharing or remote desktop during seed display
- Store the seed offline (paper/metal backup), not in digital form

## Future Hardening Roadmap

These are tracked for implementation in future releases:

- [ ] **On-screen keyboard for seed entry** — bypasses OS keyboard pipeline, accessibility loggers, predictive text
- [ ] **Seed display with dismissal gesture** — show words once, require explicit acknowledgement, then clear from DOM
- [x] ~~**Clipboard access denial for seed fields**~~ — **declined 2026-09-25.** Denying the copy button does not deny the capability (the words are selectable text), and users denied a copy photograph the screen, which is worse. Ruled the other way: the button stays and is the mitigated path — Rust-side clear after 60 s and on leaving the page, with the warning shown at the moment of copying.
- [x] **Automatic clipboard clearing** — Rust places the phrase (`copy_to_clipboard`), keeps only a SHA-256 digest, and arms a 60 s clear. Leaving the create page and destroying the window clear early, off the UI timer. A clear forgets the digest when the clipboard has changed or the OS clear succeeds, and leaves it tracked when a read or clear fails so the expiry task can retry. Another app's later clipboard value is not wiped. The webview has no clipboard capability.
- [ ] **Memory-locked allocations** — `mlock()` on pages holding wallet secrets in the Rust process
- [ ] **`prctl(PR_SET_DUMPABLE, 0)`** — suppress core dumps containing secrets on Linux

## Scanner State Persistence

The scanner state is currently in-memory only. No partial state is persisted to disk between sessions. On wallet reopen, the scanner re-scans from the wallet's last-known height. There is no `on_flush` checkpoint hook today; scan state is held by the in-process `Engine` (`engine_session.rs`, driven by `Engine::start_refresh`) pending serde support for the `(LedgerBlock, LedgerIndexes)` pair (`shekyl-engine-state` does not yet expose persistence-safe serialization). This means:

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
