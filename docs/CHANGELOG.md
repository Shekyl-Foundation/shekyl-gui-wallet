# Shekyl Wallet Changelog

## [Unreleased]

## 0.4.0-beta -- 2026-04-07

### ✨ Added

- **Real QR code generation.** Receive page now renders an actual QR code
  encoding the full FCMP++ Bech32m address using `qrcode.react`, replacing
  the previous placeholder. Uses QR error correction level M for readability
  with the ~1870-character address.
- **Fee preview on Send page.** The Send form now shows an estimated
  transaction fee (debounced 500ms) as the user enters a recipient address
  and amount, via the new `estimate_fee` Tauri command.
- **FCMP++ full-chain membership proof integration.** Replaced ring-signature
  model with full-chain membership proofs throughout the wallet. Anonymity set
  is now the entire UTXO set, displayed live in Chain Health and the new
  Security Panel.
- **Real-time proof progress pipeline.** C++ `wallet2` reports transaction
  stages (constructing, generating FCMP++ proof, PQC signing, broadcasting)
  through `i_wallet2_callback` → C FFI function pointer → Rust `mpsc` channel
  → Tauri events → React `listen` hook. Replaces cosmetic timers on Send and
  Import pages.
- **SecurityBadge header component.** Compact "3-layer — 142.8K outputs"
  badge in the header showing live FCMP++ anonymity set size. Navigates to
  Settings for full details. Replaces the PqcStatusBadge.
- **SecurityPanel component.** Detailed 3-layer protection view in Settings
  showing membership proof (FCMP++), spend authorization (Ed25519 + ML-DSA-65),
  and amount privacy (Bulletproofs+) with live curve tree statistics.
- **Dashboard security summary.** Quick-access security overview button below
  the balance card showing layer count and anonymity set size.
- **Chain Health curve tree stats.** New "Anonymity Set" counter card,
  curve tree depth row, and shortened tree root in the Chain Health panel.
- **Per-wallet staking.** Staking page now shows active/claimable stakes from
  the wallet, with functional Stake and Claim Rewards buttons.
- **Import wallet real progress.** PQC rederivation and FCMP++ tree
  precomputation stages now stream real progress from the wallet2 backend.
- **Curve tree daemon RPC.** New `get_curve_tree_info` endpoint returns leaf
  count, tree depth, and root hash.
- **Security status command.** New `get_security_status` Tauri command
  aggregates FCMP++ and PQC data into a single response.

### 🔄 Changed

- **CI targets `feature/fcmp-plus-plus` branch.** Both `ci.yml` and
  `release.yml` now clone shekyl-core's `feature/fcmp-plus-plus` branch.
- **Build links FCMP++ libraries.** `build.rs` now links `libfcmp.a` and
  `libfcmp_basic.a` instead of the removed `libringct.a` / `libringct_basic.a`.
- **Help Center restructured.** Renamed "Post-Quantum Cryptography" section
  to "Security and Privacy" with "Three Layers of Protection" content covering
  FCMP++, PQC, and Bulletproofs+. Added FCMP++ and Curve Tree glossary entries.
- **Transaction history enhanced.** Rows now display fee, formatted timestamp,
  and confirmation status.
- **DaemonContext fetches security data.** Polling now includes
  `get_security_status` alongside chain health and PQC status.

### 🗑️ Removed

- **PqcStatusBadge component.** Replaced by SecurityBadge with richer
  FCMP++ awareness.
- **Cosmetic progress timers.** Send page and Import page no longer use
  `setTimeout` chains; all progress is real.

## 0.3.0-beta -- 2026-04-03

### ✨ Added

- **Windows MSVC build support**: The release CI now builds shekyl-core
  from source using Visual Studio 2026 / MSVC on `windows-2025-vs2026`,
  with vcpkg for C++ dependencies and lmdb overlay ports from shekyl-core.
  Builds only the `wallet_api` target with `--parallel 2` to stay within
  CI memory limits and avoid MSVC ICE issues.
- **`build.rs` Windows linkage**: Added vcpkg search paths, static Boost
  / OpenSSL / libsodium / protobuf linking, MSVC `Release/` subdirectory
  search paths, and `wallet-crypto.lib` detection for MSVC builds.
- **Bech32m address display (Phase 5.7a)**: Receive page now splits FCMP++
  addresses into classical and post-quantum segments. Classical segment shown
  by default with a "Show full address" toggle. Copy-to-clipboard always
  copies the full address. QR code placeholder annotated to encode the full
  FCMP++ address. FCMP++ badge displayed when a PQ segment is detected.
- **PQC-protected transaction indicators (Phase 5.7b)**: Transaction history
  shows a green "PQC" shield badge on outputs protected by FCMP++ membership
  proofs with post-quantum signatures. Added `pqc_protected` field to `TxInfo`
  in both the Rust backend (`commands.rs`, `wallet_bridge.rs`) and the
  TypeScript frontend.
- **FCMP++ proof generation progress (Phase 5.7b)**: Send page now shows a
  multi-stage progress indicator during transaction construction:
  "Constructing transaction" -> "Generating FCMP++ membership proof" ->
  "Applying hybrid PQC signature" -> "Broadcasting to network", with a
  progress bar and contextual PQC protection message.
- **Restore-from-seed progress stages (Phase 5.7c)**: Import/restore wallet
  flow now shows a stepped progress UI: "Scanning blockchain..." ->
  "Rederiving quantum-safe keys..." -> "Building FCMP++ tree state..." ->
  "Restore complete", with checkmarks for completed stages and an animated
  progress bar. Form is hidden during restore to reduce visual noise.

### 📚 Documentation

- **FCMP++ documentation rework.** Updated user-facing privacy documentation
  to reflect FCMP++ full-chain membership proofs replacing ring signatures.
  Updated glossary, address format (Bech32m `shekyl1:`), and future roadmap
  language. Updated WALLET_STARTUP.md with FCMP++ and ML-KEM-768 details.

### 🔄 Changed

- **CI targets shekyl-core `dev` branch**: Both `ci.yml` and `release.yml`
  now clone shekyl-core's `dev` branch (Rust/Axum RPC, PQC multisig, MSVC
  portability) instead of `main`. This is temporary for test builds.
- **Removed classical multisig link**: `build.rs` no longer links `libmultisig.a`
  or searches `src/multisig/` -- classical multisig was removed from shekyl-core;
  PQC multisig is implemented entirely in Rust.

## 0.1.4-beta -- 2026-04-01

### 🔄 Changed

- **Static linking via `contrib/depends`**: Linux release builds now use
  shekyl-core's `contrib/depends` system to build all third-party libraries
  (Boost, OpenSSL, libsodium, protobuf, libunbound, hidapi, libusb, zeromq)
  from source with static linking. The `.deb` package only depends on
  `libwebkit2gtk-4.1-0`, `libayatana-appindicator3-1`, and `libudev1`. A single
  universal `.deb` replaces the per-distro packages.
- **`build.rs` static linking support**: New `SHEKYL_DEPENDS_PREFIX` env var
  enables static linking of all third-party libraries from the depends prefix.
  Falls back to dynamic linking for local development when unset.
- **Collapsed Ubuntu CI matrix**: Removed the `ubuntu-22.04` / `ubuntu-24.04`
  dual-build and the `sed`-based dependency patching. A single `ubuntu-24.04`
  runner produces one universal `.deb` + one `.AppImage`.

### 🗑️ Removed

- Per-distro `.deb` dependency lists (Boost 1.74/1.83, libssl3/libssl3t64, etc.)
  -- no longer needed with static linking.

## 0.1.3-beta -- 2026-04-01

### 🔄 Changed

- **Multi-distro `.deb` packages**: Release workflow now builds separate `.deb`
  files for Ubuntu 22.04 and 24.04 with correct versioned dependencies for each
  (Boost 1.74 vs 1.83, `libssl3` vs `libssl3t64`, `libprotobuf23` vs
  `libprotobuf32t64`). Files are suffixed with the distro version
  (e.g., `_ubuntu-22.04.deb`, `_ubuntu-24.04.deb`). AppImage remains the
  universal Linux option.
- **Release workflow restructured**: A `create-release` job now creates the
  GitHub release first; Linux builds use `npx tauri build` with manual artifact
  upload (distro-suffixed `.deb` names), while macOS and Windows continue using
  `tauri-action` with `releaseId`.

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

### Direct wallet2 FFI Integration

- Replaced the HTTP JSON-RPC client (`wallet_rpc.rs`) and child process manager
  (`wallet_process.rs`) with `wallet_bridge.rs`, which calls the C++ `wallet2`
  library directly through the `shekyl-wallet-rpc` Rust FFI wrapper.
- Updated `state.rs`: `AppState` now holds a `wallet_bridge::WalletHandle`
  instead of `wallet_process` and `wallet_rpc_url`.
- Updated `commands.rs`: All wallet commands now call `wallet_bridge` functions
  instead of `wallet_rpc`.
- Updated `lib.rs`: Replaced `mod wallet_process` and `mod wallet_rpc` with
  `mod wallet_bridge`.
- Removed `wallet_rpc.rs`, `wallet_process.rs`, and `which` crate dependency.
- Added `wallet_bridge.rs` and `shekyl-wallet-rpc` path dependency.

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

### Build & CI

- **Release workflow now builds shekyl-core from source** for Linux and macOS.
- **`build.rs` rewritten** with comprehensive platform-conditional linking:
  24 static libraries plus the `shekyl_ffi` Rust FFI crate, with
  platform-specific system library lists for Linux, macOS, and Windows.
- **macOS Homebrew detection**: `build.rs` discovers Homebrew prefix at build
  time via `brew --prefix` and conditionally links only Boost components that
  produce library files.
- **Windows MSVC build enabled**: shekyl-core `dev` branch contains all MSVC
  portability fixes (POSIX guards, CryptonightR JIT stub, empty `.dat`
  generator fix, Boost CONFIG-mode detection). Uses `windows-2025-vs2026`
  runner with VS 2026, vcpkg overlay ports for lmdb, and builds only
  `wallet_api` with `--parallel 2`.

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
  controls (address input, thread slider 1-N, background mining toggle,
  start/stop buttons), privacy note about 60-block coinbase maturity.
- Added `MiningStatus` TypeScript interface to `types/daemon.ts`.

### Help Center

- Created `Help.tsx` page with five expandable sections: Getting Started,
  Mining Guide, Staking Guide, Post-Quantum Cryptography, and Glossary.
- PQC section covers: quantum threat explanation, hybrid Ed25519 + ML-DSA-65,
  FCMP++ full-chain membership proofs, per-output PQC keys (X25519 + ML-KEM-768).
- Glossary with 11 terms including FCMP++ membership proof, ML-DSA-65, RandomX.

### Contextual Help and Tooltips

- PQC status badge in header now links to Help page on click.
- Added info tooltip to compact ChainHealthPanel ("What is Chain Health?").
- Added info tooltip to Mining page hash rate speed display.

### Navigation

- Added Mining (Pickaxe icon) and Help (HelpCircle icon) to sidebar navigation.
- Added `/mining` and `/help` routes to `App.tsx`.

### Testing

- Frontend: 65 tests across 13 suites (up from 49/11).
- Rust backend: 10 unit tests.

### Daemon RPC Integration

- Added `daemon_rpc.rs`: async JSON-RPC client for `shekyld` wrapping
  `get_info`, `get_staking_info`, `estimate_claim_reward`, and
  `get_last_block_header`. Uses `reqwest` with `rustls` TLS backend.
- Added `state.rs`: Tauri managed state holding daemon URL, network type,
  and shared HTTP client. Supports MainNet (11029), TestNet (12029), and
  StageNet (13029) with automatic port defaults.

### SKL Display Formatting

- Created `src/lib/format.ts`: canonical formatting utilities with 9-decimal
  atomic precision (1e9 divisor) and 6-decimal display truncation.
- Fixed incorrect 1e12 (Monero) divisor throughout the codebase.

### Chain Health Dashboard

- Added `ChainHealthPanel.tsx`: card-based dashboard showing emission era,
  supply progress bar, stake ratio / release tempo / burn rate / emission
  share gauges, total burned / staker pool / total staked counters, and
  network stats (height, hash rate, last reward, tx pool, node version).
- Added `EmissionGauge.tsx`: reusable SVG circular gauge component.

### Enhanced Staking View

- Rebuilt `Staking.tsx` with privacy-first narrative.
- `StakeTierCard.tsx`: tier selection cards showing lock duration, yield
  multiplier, estimated APY, and block count from live daemon data.

### PQC Transparency (superseded by SecurityBadge / SecurityPanel)

- Added `PqcStatusBadge.tsx` to the header: shows "Ed25519 + ML-DSA-65"
  with green shield icon when hybrid PQC is active.
- Added "Post-Quantum Security" section to Settings page.

### Network Switching and Testnet Flow

- Added `TestnetBanner.tsx`: persistent amber banner on TestNet, blue on StageNet.
- Network selector in Settings now calls `set_daemon_connection` to switch
  daemon URL and network at runtime, with immediate data refresh.

### Shared Polling Context

- Added `DaemonProvider` context: polls `get_chain_health` and
  `get_pqc_status` every 30 seconds.

### Testing and CI/CD

- Added frontend testing: Vitest + React Testing Library + jsdom.
- Added Rust backend tests: `#[tokio::test]` unit tests.
- Added CI workflow and release workflow with multi-platform matrix.

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
