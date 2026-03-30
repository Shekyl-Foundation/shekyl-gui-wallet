# Shekyl Wallet Changelog

## Unreleased

### Testing and CI/CD

- Added frontend testing: Vitest + React Testing Library + jsdom (20 tests,
  6 suites) covering BalanceCard, NetworkBadge, Sidebar, Dashboard, Send,
  and Receive pages. Tauri IPC mocked via `@tauri-apps/api/mocks`.
- Added Rust backend tests: 10 `#[tokio::test]` unit tests for all command
  stubs in `commands.rs`.
- Added CI workflow (`.github/workflows/ci.yml`): ESLint, TypeScript
  type-check, Vitest, Rustfmt, Clippy, and `cargo test` on every PR to main.
- Added release workflow (`.github/workflows/release.yml`): multi-platform
  build matrix (Linux x64, Windows x64, macOS ARM64, macOS Intel) via
  `tauri-apps/tauri-action`, creates draft GitHub releases.
- Added `rustfmt.toml`, `vitest.config.ts`, and test setup infrastructure.
- Added `test`, `test:watch`, `test:coverage`, and `typecheck` npm scripts.
- Fixed ESLint to ignore `src-tauri/` build artifacts.

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
