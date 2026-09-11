# Contributing to the Shekyl GUI Wallet

## Architecture

The GUI wallet is a Tauri 2 application:

- **Frontend**: React + TypeScript + Tailwind CSS (in `src/`)
- **Backend**: Rust Tauri commands (in `src-tauri/src/`)
- **Core**: pure-Rust `shekyl-engine-core::Engine`, embedded in-process
  (`src-tauri/src/engine_session.rs`) + Rust `shekyl-scanner`

## Development Setup

1. Install Node.js (version pinned in `.nvmrc`): `nvm use`
2. Install Rust (version pinned in `src-tauri/rust-toolchain.toml`)
3. Install dependencies: `npm install`
4. Run in development: `npm run tauri dev`

## Branching Model

This repo mirrors shekyl-core: **`main` = stable, `dev` = integration**.
Policy: `.cursor/rules/06-branching.mdc`.

- **`main`** is stable. It only advances via a **merge commit** from `dev`
  (`git merge --no-ff dev` / GitHub "Create a merge commit"). No feature
  work, no direct commits. Fast-forward, squash, and rebase dev→main are
  forbidden — they elide the release boundary or fail once `main` has any
  commit not on `dev`.
- **`dev`** is integration. All work branches off dev and merges back to
  dev. Version bumps and CHANGELOG cuts happen on dev.
- **CI**: dev CI (`ci.yml`, `codeql.yml`) tracks shekyl-core `dev`.
  Release builds (`release.yml`) clone the matching shekyl-core tag
  (`SHEKYL_CORE_REF`, currently `v3.1.0-alpha.8`).
- **Release flow** (same shape as shekyl-core):
  1. dev is verified.
  2. Open a PR dev → main titled `Release: vX.Y.Z` (audit trail even
     with one maintainer).
  3. Merge with **Create a merge commit** (never FF / squash / rebase).
  4. Signed annotated tag on **that merge commit**.
  5. Push the tag; `release.yml` builds the installers.
  6. If dev has not moved, reverse-FF dev to the merge
     (`git merge --ff-only main` on dev). dev is not rebased onto main.
- **`v3.1.0-alpha.8` exception:** that tag was signed on dev (`7d209ad`)
  because `main` still carried the April 2026 duplicate-history split.
  The dev→main merge that closed the split does **not** move the tag.
  Subsequent releases tag the merge commit on `main`.
- **Never commit infrastructure directly to `main`**. The April 2026
  split (`main` ahead of dev with infra SHAs dev did not share) is
  closed by the dev→main merge: dev's tree wins; `main`'s unique
  commits remain as first-parent history (append-only, no force-push).

## Code Guidelines

### Input Validation

Every Tauri command that accepts user input MUST validate through
`src-tauri/src/validate.rs` before reaching the wallet FFI.
See `docs/GUI_SECURITY.md` for the full validation table.

### Secret Key Handling

- All secrets are wrapped in `Zeroizing<T>` on the Rust side.
- The scanner state (`(LedgerBlock, LedgerIndexes)` from
  `shekyl-engine-state`) and `TransferDetails` implement `ZeroizeOnDrop`.
- `close_wallet` and `shutdown` wipe scanner state explicitly (the
  `Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>` is reset to the
  empty pair, dropping the old secrets through `Zeroize`).
- Never log, serialize to plaintext, or return secrets in error messages.

### Testing

**Rust tests**: `cargo test -p shekyl-wallet` (run from `src-tauri/`)

**Frontend tests**: `npm test` (Vitest)

**Canary leak tests** (Gate 6): validate.rs includes tests that plant known
canary patterns in secret-like inputs and assert error messages don't leak them.

**Unit vs. sidecar-integration tests**: the backend unit tests run on every
push/PR (`ci.yml` → `cargo test`) and must not need a running daemon. A test
that requires a live `shekyld` sidecar (spawning it, hitting its RPC) is an
_integration_ test: mark it `#[ignore = "requires live shekyld sidecar"]`.
`cargo test` skips `#[ignore]` by default, so CI stays fast and daemon-free;
the release build (`release.yml`) compiles the real sidecar and runs exactly
those tests via `cargo test --release -- --ignored`. Run them locally with a
sidecar present the same way. Keep pure logic in unit tests wherever possible —
reserve `#[ignore]` for the cases that genuinely cannot be exercised without a
daemon.

### Security

See `docs/GUI_SECURITY.md` for the threat model, CSP policy, and hardening
checklist. All PRs that touch validation, commands, or wallet bridge must be
reviewed against that document.

## Supply Chain

- Node.js version: pinned in `.nvmrc`
- Rust toolchain: pinned in `src-tauri/rust-toolchain.toml`
- Dependency auditing: `.github/workflows/audit.yml` runs `npm audit` and `cargo audit`
- `package-lock.json` and `Cargo.lock` are committed and must be kept up to date.

### npm Dependency Review (PR Checklist)

Any PR that modifies `package.json` or `package-lock.json` must satisfy:

1. **Justification**: Explain why the new dependency is needed and why an existing
   dependency or built-in API cannot serve the purpose.
2. **`npm audit`**: `npm audit --audit-level=moderate` must pass with zero findings.
   If a finding exists, document the risk assessment and mitigation in the PR.
3. **License check**: New packages must use permissible licenses (MIT, ISC, BSD-2,
   BSD-3, Apache-2.0). Copyleft (GPL, LGPL, AGPL) requires explicit approval.
4. **Size impact**: Run `npm run build` before and after; report the bundle-size
   delta. Dependencies that increase the production bundle by >50 KB require
   justification.
5. **Maintenance health**: Prefer packages with >1,000 weekly downloads, recent
   commits within the last 6 months, and no open critical CVEs.
6. **Lock file integrity**: `package-lock.json` changes must be generated by
   `npm install`, never hand-edited. CI verifies with `npm ci`.
