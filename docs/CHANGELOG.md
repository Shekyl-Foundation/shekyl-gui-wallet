# Shekyl Wallet Changelog

## 0.1.3-beta -- 2026-04-01

### 🔄 Changed

- **Direct wallet2 FFI integration**: Replaced the HTTP JSON-RPC client
  (`wallet_rpc.rs`) and child process manager (`wallet_process.rs`) with
  `wallet_bridge.rs`, which calls the C++ `wallet2` library directly through
  the `shekyl-wallet-rpc` Rust FFI wrapper. The GUI wallet no longer spawns a
  separate `shekyl-wallet-rpc` process or communicates over HTTP — all wallet
  operations happen in-process via the C FFI facade.
- **Updated `state.rs`**: `AppState` now holds a `wallet_bridge::WalletHandle`
  (`Mutex<Option<Wallet2>>`) instead of `wallet_process: Mutex<Option<Child>>`
  and `wallet_rpc_url`. Removed `wallet_url()` method; added
  `daemon_address()` for wallet2 init.
- **Updated `commands.rs`**: All wallet commands (`create_wallet`,
  `open_wallet`, `close_wallet`, `get_balance`, `get_address`, `transfer`,
  `get_transactions`, `get_seed`, `import_wallet_from_seed`,
  `import_wallet_from_keys`) now call `wallet_bridge` functions instead of
  `wallet_rpc`. `init_wallet_rpc` creates a `Wallet2` instance directly.
  `shutdown_wallet_rpc` drops the instance.
- **Updated `lib.rs`**: Replaced `mod wallet_process` and `mod wallet_rpc` with
  `mod wallet_bridge`. Window close handler calls `wallet_bridge::shutdown`.

### 🗑️ Removed

- `wallet_rpc.rs` — async HTTP JSON-RPC client (replaced by direct FFI)
- `wallet_process.rs` — child process spawner/manager (no longer needed)
- `which` crate dependency (was used for binary resolution)

### ✨ Added

- `wallet_bridge.rs` — direct FFI bridge to wallet2 via `shekyl-wallet-rpc`
  crate. Provides the same API surface as the old `wallet_rpc.rs` but without
  HTTP overhead or process management.
- `shekyl-wallet-rpc` path dependency in `Cargo.toml`
- `build.rs` with `SHEKYL_BUILD_DIR` support for linking C++ wallet libraries

### 🔧 Build & CI

- **Release workflow now builds shekyl-core from source** for Linux and macOS.
  The workflow installs all C++ build dependencies (Boost, OpenSSL, libsodium,
  protobuf, etc.), configures and builds the shekyl-core wallet library via
  CMake, and passes `SHEKYL_BUILD_DIR` to Tauri's build step so the Rust
  linker can resolve all 22 `wallet2_ffi_*` symbols.
- **`build.rs` rewritten** with comprehensive platform-conditional linking:
  24 static libraries (wallet, cryptonote_core, blockchain_db, ringct, etc.)
  plus the `shekyl_ffi` Rust FFI crate, with platform-specific system library
  lists for Linux (`stdc++`, Boost, OpenSSL, hidapi, udev), macOS (`c++`,
  frameworks: Security, CoreFoundation, IOKit), and Windows (ws2_32, bcrypt).
- **macOS Homebrew detection**: `build.rs` discovers Homebrew prefix at build
  time via `brew --prefix` and `brew --prefix openssl@3`, and conditionally
  links only Boost components that produce library files (Boost.System is
  header-only in Boost 1.69+).
- **Linux `.deb` runtime dependencies**: declared all 14 runtime library
  dependencies (Boost, OpenSSL, libsodium, libunbound, hidapi, protobuf, etc.)
  so `apt install` pulls them automatically on Ubuntu 22.04. For cross-distro
  Linux support, use the AppImage which bundles all shared libraries.
- **shekyl-core cloned from `main`**: CI workflows now pull shekyl-core from
  the `main` branch (stable) rather than `dev`.
- **Windows**: deferred pending MSVC compatibility investigation. The current
  shekyl-core codebase uses GCC/MinGW which produces `.a` archives
  incompatible with MSVC's `link.exe`. Key issues identified:
  unconditional POSIX includes in `common/util.cpp` and `easylogging++.cc`,
  AT&T inline asm in `perf_timer.cpp`, and GCC builtins in crypto code.

## 0.1.2-beta -- 2026-03-30

### Wallet Startup Flow

- Added wallet startup state machine: app detects existing `.keys` files in
  the platform default wallet directory on launch. If a wallet is found, the
  user is presented with an unlock (password) screen; if not, a welcome screen
  offers Create New Wallet or Import Existing Wallet.
- Added `wallet_rpc.rs`: async JSON-RPC client for `shekyl-wallet-rpc`,
  mirroring `daemon_rpc.rs`. Covers `create_wallet`, `open_wallet`,
  `close_wallet`, `restore_deterministic_wallet`, `generate_from_keys`,
  `get_address`, `get_balance`, `query_key`, `get_version`, `stop_wallet`,
  `get_transfers`, and `transfer`.
- Added `wallet_process.rs`: manages `shekyl-wallet-rpc` as a child process.
  Layered binary resolution (Tauri sidecar > PATH > user config), process
  spawn in `--wallet-dir` mode, readiness polling, and graceful shutdown.
- Updated `state.rs`: `AppState` now includes `wallet_rpc_url`, `wallet_dir`,
  `wallet_open`, `wallet_name`, and `wallet_process` fields. Platform-specific
  default wallet directories (Linux: `~/.shekyl/wallets/`, macOS:
  `~/Library/Application Support/shekyl/wallets/`, Windows:
  `%APPDATA%\shekyl\wallets\`). Wallet-RPC port defaults to daemon port + 1.
- Replaced all wallet command stubs in `commands.rs` with real implementations
  that call through `wallet_rpc.rs`. New commands: `check_wallet_files`,
  `init_wallet_rpc`, `shutdown_wallet_rpc`, `import_wallet_from_seed`,
  `import_wallet_from_keys`, `get_seed`.
- Updated `lib.rs`: registered new modules and commands; added window close
  hook for wallet-rpc process cleanup.
- Added `dirs` and `which` crate dependencies.
- Created `WalletProvider` React context with phase-based state machine
  (`loading` / `no_wallet` / `select_wallet` / `unlock` / `ready`).
- Created four new pages: `Welcome.tsx` (first-launch onboarding),
  `CreateWallet.tsx` (4-step wizard: name+password, seed display, seed
  confirmation, success), `ImportWallet.tsx` (tabbed seed-phrase and key-based
  restore), `Unlock.tsx` (password entry with multi-wallet selector).
- Created `LoadingScreen.tsx`: animated Shekyl branding shown during wallet-rpc
  startup.
- Refactored `App.tsx` routing: wallet-gated architecture that shows onboarding
  flow until a wallet is open, then renders the main Layout with sidebar.
  `DaemonProvider` now only mounts when wallet is ready.
- Added Tauri sidecar scaffold: `src-tauri/binaries/` directory with README
  documenting target-triple naming convention and `externalBin` configuration
  for future bundling of `shekyld` and `shekyl-wallet-rpc`.
- Created `docs/WALLET_STARTUP.md` design doc.

### Release Pipeline

- Changed `releaseDraft: true` to `releaseDraft: false` in `release.yml` so
  tagged releases are published automatically. The `prerelease` flag already
  handles beta/alpha/rc marking.

### Version Sync

- Added `scripts/sync-version.mjs`: reads version from `package.json` and
  propagates to `Cargo.toml` and `tauri.conf.json` in a single command.
- Added `npm run version:bump` and `npm run version:sync` scripts.
- Sidebar version string now reads `__APP_VERSION__` from Vite's build-time
  `define`, eliminating the fourth manual version update.
- Added `__APP_VERSION__` global declaration to `src/vite-env.d.ts`.

### Mining Page and Backend

- Added plain HTTP endpoint helpers to `daemon_rpc.rs`: `http_json_call`,
  `mining_status`, `start_mining`, `stop_mining` for the daemon's
  `/mining_status`, `/start_mining`, `/stop_mining` endpoints.
- Added `MiningStatusResponse` struct with fields: `active`, `speed`,
  `threads_count`, `address`, `pow_algorithm`, `is_background_mining_enabled`,
  `block_target`, `block_reward`, `difficulty`.
- Added `AppState::base_url()` to `state.rs` for stripping `/json_rpc` suffix,
  needed by mining HTTP endpoints.
- Added Tauri commands: `get_mining_status`, `start_mining_cmd`, `stop_mining_cmd`.
- Created `Mining.tsx` page with: live status card (active/idle indicator,
  hash rate gauge using `EmissionGauge`, thread count, speed, algorithm),
  network mining context (difficulty, block reward, block time), mining
  controls (address input, thread slider 1–N, background mining toggle,
  start/stop buttons), privacy note about 60-block coinbase maturity.
- Added `MiningStatus` TypeScript interface to `types/daemon.ts`.

### Help Center

- Created `Help.tsx` page with five expandable sections: Getting Started,
  Mining Guide, Staking Guide, Post-Quantum Cryptography, and Glossary.
- Getting Started covers: what is Shekyl, connecting to daemon, creating a
  wallet, sending and receiving.
- Mining Guide covers: what is mining (RandomX PoW), how to mine in-wallet,
  block rewards and 60-block unlock, requirements (unrestricted daemon).
- Staking Guide covers: claim-based model (not delegation), tiers and lock
  durations, privacy benefit (accrual pool commingling), estimated APY.
- PQC section covers: quantum threat explanation, hybrid Ed25519 + ML-DSA-65,
  what "hybrid" means (belt-and-suspenders), V4 roadmap (lattice rings + KEM
  stealth addresses), coinbase transaction exemption.
- Glossary with 11 terms: atomic unit, block height, difficulty, emission era,
  hash rate, ML-DSA-65, RandomX, release multiplier, ring signature, stake
  ratio, stealth address.

### Contextual Help and Tooltips

- PQC status badge in header now links to Help page on click.
- Added info tooltip to compact ChainHealthPanel ("What is Chain Health?").
- Added info tooltip to Mining page hash rate speed display.

### Navigation

- Added Mining (Pickaxe icon) and Help (HelpCircle icon) to sidebar navigation.
- Added `/mining` and `/help` routes to `App.tsx`.

### Testing

- Frontend: 65 tests across 13 suites (up from 49/11).
  New suites: `Mining.test.tsx` (8 tests), `Help.test.tsx` (8 tests).
  Updated `Sidebar.test.tsx` for Mining and Help links.
  Updated `PqcStatusBadge.test.tsx` for router context.
- Rust backend: 10 unit tests (added `get_staking_info_returns_ffi_message`).

### Daemon RPC Integration

- Added `daemon_rpc.rs`: async JSON-RPC client for `shekyld` wrapping
  `get_info`, `get_staking_info`, `estimate_claim_reward`, and
  `get_last_block_header`. Uses `reqwest` with `rustls` TLS backend.
- Added `state.rs`: Tauri managed state holding daemon URL, network type,
  and shared HTTP client. Supports MainNet (11029), TestNet (12029), and
  StageNet (13029) with automatic port defaults.
- Replaced mock `get_wallet_status` stub with real daemon connectivity
  check via `get_info`.
- Added new Tauri commands: `get_chain_health` (aggregated daemon data),
  `get_tier_yields` (annualized APY per staking tier), `set_daemon_connection`
  (runtime network/URL switching), `get_pqc_status` (hybrid signature info).
- Wallet-side commands (transfer, stake, claim, create/open wallet) remain
  stubs with clear "requires wallet2 FFI bridge" messages.

### SKL Display Formatting

- Created `src/lib/format.ts`: canonical formatting utilities with 9-decimal
  atomic precision (1e9 divisor) and 6-decimal display truncation.
- Functions: `formatSkl`, `formatSklCompact`, `formatPercent`,
  `formatMultiplier`, `formatHashRate`, `formatDuration`, `emissionProgress`.
- Fixed incorrect 1e12 (Monero) divisor throughout the codebase.
- Created `src/types/daemon.ts`: shared TypeScript interfaces for all daemon
  RPC response shapes.

### Chain Health Dashboard (DESIGN_CONCEPTS.md Section 10)

- Added `ChainHealthPanel.tsx`: card-based dashboard showing emission era,
  supply progress bar, stake ratio / release tempo / burn rate / emission
  share gauges, total burned / staker pool / total staked counters, and
  network stats (height, hash rate, last reward, tx pool, node version).
- Added `EmissionGauge.tsx`: reusable SVG circular gauge component (pure
  CSS/SVG, no charting library).
- Added `/chain-health` route with full-page view; compact version embedded
  on the Dashboard below balance.
- Added "Chain Health" link to sidebar navigation.

### Enhanced Staking View

- Rebuilt `Staking.tsx` with privacy-first narrative: prominent "Staking as
  Privacy Participation" card explaining accrual pool commingling and
  plausible deniability.
- Network staking gauges: stake ratio, emission share, total staked, and
  reward pool balance.
- `StakeTierCard.tsx`: tier selection cards showing lock duration, yield
  multiplier, estimated APY, and block count from live daemon data.
- Stake action section with wallet2 FFI deferral notice.

### PQC Transparency

- Added `PqcStatusBadge.tsx` to the header: shows "Ed25519 + ML-DSA-65"
  with green shield icon when hybrid PQC is active.
- Added "Post-Quantum Security" section to Settings page showing signature
  scheme, transaction version, protection status, and V4 roadmap note
  (lattice ring signatures + KEM stealth addresses).

### Network Switching and Testnet Flow

- Added `TestnetBanner.tsx`: persistent amber banner on TestNet ("TestNet
  Active — Mainnet July 2026" + faucet link), blue banner on StageNet.
- Network selector in Settings now calls `set_daemon_connection` to switch
  daemon URL and network at runtime, with immediate data refresh.
- Settings page shows live connection status (block height + node version).

### Shared Polling Context

- Added `DaemonProvider` context: polls `get_chain_health` and
  `get_pqc_status` every 30 seconds. All components subscribe to the
  context, avoiding duplicate RPC traffic.
- Header now uses context for connection/sync status display.

### Testing

- Frontend: 49 tests across 11 suites (up from 20 tests / 6 suites).
  New test coverage for: `format.ts` utilities, `EmissionGauge`,
  `PqcStatusBadge`, `TestnetBanner`, `StakeTierCard`, and updated
  existing tests for refactored components.
- Rust backend: 9 unit tests covering all stub commands and PQC status.
- All checks pass: TypeScript, ESLint, Vitest, Rustfmt, Clippy, cargo test.

### Testing and CI/CD

- Added frontend testing: Vitest + React Testing Library + jsdom. Tauri IPC
  mocked via `@tauri-apps/api/mocks`.
- Added Rust backend tests: `#[tokio::test]` unit tests for command stubs.
- Added CI workflow (`.github/workflows/ci.yml`): ESLint, TypeScript
  type-check, Vitest, Rustfmt, Clippy, and `cargo test` on every PR to main.
- Added release workflow (`.github/workflows/release.yml`): multi-platform
  build matrix (Linux x64, Windows x64, macOS ARM64, macOS Intel) via
  `tauri-apps/tauri-action`, creates draft GitHub releases.
- Added `rustfmt.toml`, `vitest.config.ts`, and test setup infrastructure.

## 0.1.0 -- 2026-03-14

### Initial scaffold

- Tauri 2 project with Vite + React 19 + TypeScript + Tailwind CSS 4.
- Six pages: Dashboard, Send, Receive, Staking, Transactions, Settings.
- Stub Tauri commands returning mock data (Phase 2: real wallet2 FFI bridge).
- Shekyl gold/purple design system ported from shekyl-web.
- Sidebar navigation with Lucide icons.
- Branding assets: `shekyl_icon_v2.svg`, `shekyl_symbol.svg`.
- User installation guide (`docs/INSTALLATION.md`) for Linux, Windows, macOS.
- Published to [Shekyl-Foundation/shekyl-gui-wallet](https://github.com/Shekyl-Foundation/shekyl-gui-wallet).
