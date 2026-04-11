# Contributing to the Shekyl GUI Wallet

## Architecture

The GUI wallet is a Tauri 2 application:

- **Frontend**: React + TypeScript + Tailwind CSS (in `src/`)
- **Backend**: Rust Tauri commands (in `src-tauri/src/`)
- **Core**: C++ `wallet2` FFI via `shekyl-wallet-rpc` + Rust `shekyl-scanner`

## Development Setup

1. Install Node.js (version pinned in `.nvmrc`): `nvm use`
2. Install Rust (version pinned in `src-tauri/rust-toolchain.toml`)
3. Install dependencies: `npm install`
4. Run in development: `npm run tauri dev`

## Code Guidelines

### Input Validation

Every Tauri command that accepts user input MUST validate through
`src-tauri/src/validate.rs` before reaching the wallet FFI.
See `docs/GUI_SECURITY.md` for the full validation table.

### Secret Key Handling

- All secrets are wrapped in `Zeroizing<T>` on the Rust side.
- `WalletState` and `TransferDetails` implement `ZeroizeOnDrop`.
- `close_wallet` and `shutdown` wipe scanner state explicitly.
- Never log, serialize to plaintext, or return secrets in error messages.

### Testing

**Rust tests**: `cargo test -p shekyl-wallet` (run from `src-tauri/`)

**Frontend tests**: `npm test` (Vitest)

**Canary leak tests** (Gate 6): validate.rs includes tests that plant known
canary patterns in secret-like inputs and assert error messages don't leak them.

### Security

See `docs/GUI_SECURITY.md` for the threat model, CSP policy, and hardening
checklist. All PRs that touch validation, commands, or wallet bridge must be
reviewed against that document.

## Supply Chain

- Node.js version: pinned in `.nvmrc`
- Rust toolchain: pinned in `src-tauri/rust-toolchain.toml`
- Dependency auditing: `.github/workflows/audit.yml` runs `npm audit` and `cargo audit`
- `package-lock.json` and `Cargo.lock` are committed and must be kept up to date.
