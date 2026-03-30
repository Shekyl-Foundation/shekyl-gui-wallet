# Shekyl Wallet Changelog

## Unreleased

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
