# Shekyl GUI Wallet

Cross-platform desktop wallet for Shekyl (SKL), built with **Tauri 2**, **React**, and **Rust**.

## Features

- **Live chain health dashboard** — emission era, release tempo, burn rate,
  stake ratio, total burned/staked, hash rate, and block reward from the
  daemon, matching the DESIGN_CONCEPTS.md Section 10 spec
- **Staking with privacy narrative** — tier selection (Tier 0/1/2) with live
  APY estimates, accrual pool commingling explanation, and plausible
  deniability on claims
- **Post-quantum transparency** — header badge showing hybrid Ed25519 +
  ML-DSA-65 protection status, with PQC details and V4 roadmap in Settings
- **Network switching** — MainNet / TestNet / StageNet with automatic daemon
  URL configuration, persistent testnet banners, and faucet links
- **9-decimal canonical SKL display** — 6-decimal readability with full
  precision available
- Create, open, and manage Shekyl wallets (stubs — wallet2 FFI pending)
- Send and receive SKL (stubs)
- Transaction history
- Modern dark UI with the Shekyl gold & purple design system

## Architecture

```
Frontend:  Vite + React 19 + TypeScript + Tailwind CSS 4
Backend:   Rust (Tauri 2) — JSON-RPC client to shekyld daemon
IPC:       Tauri invoke bridge
TLS:       rustls (no OpenSSL dependency)
Future:    C++ FFI bridge to wallet2_api.h (shekyl-core)
```

The Rust backend connects to a `shekyld` daemon via JSON-RPC for chain health,
staking, and economic data. Wallet-side operations (create, transfer, stake,
claim) remain stubs until the wallet2 C++ FFI bridge is implemented.

## Prerequisites

### All platforms

- [Rust](https://rustup.rs/) >= 1.77
- [Node.js](https://nodejs.org/) >= 20
- npm >= 10

### Linux (Debian / Ubuntu)

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget \
  file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### macOS

Xcode command line tools are required:

```bash
xcode-select --install
```

### Windows

- [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included in Windows 11)

## Development

```bash
# Install dependencies
npm install

# Start development mode (hot-reload frontend + Rust backend)
npm run tauri dev

# Type-check the frontend
npm run typecheck

# Lint
npm run lint
```

## Testing

### Frontend (Vitest + React Testing Library)

```bash
npm test                # Single run (49 tests, 11 suites)
npm run test:watch      # Watch mode
npm run test:coverage   # With V8 coverage report
```

Tests live alongside the code in `__tests__/` directories. Tauri IPC calls
are mocked via `@tauri-apps/api/mocks` so tests run without a Rust backend.

### Rust backend (cargo test)

```bash
cd src-tauri
cargo test              # 9 tests
```

### Linting

```bash
npm run lint                              # ESLint
npm run typecheck                         # TypeScript
cd src-tauri && cargo fmt --check         # Rustfmt
cd src-tauri && cargo clippy -- -D warnings  # Clippy
```

## CI/CD

The project uses GitHub Actions with two workflows:

- **CI** (`.github/workflows/ci.yml`) — runs on every push/PR to `main`:
  ESLint, TypeScript type-check, Vitest, Rustfmt, Clippy, and cargo test.
- **Release** (`.github/workflows/release.yml`) — runs on push to the
  `release` branch: builds platform-specific binaries (Linux, Windows,
  macOS ARM64 + Intel) via `tauri-action` and creates a draft GitHub release
  with all artifacts attached.

## Build

```bash
# Production build (generates platform-specific bundles)
npm run tauri build
```

Build artifacts are placed in `src-tauri/target/release/bundle/`:

| Platform | Format |
|----------|--------|
| Linux | `.deb`, `.rpm`, `.AppImage` |
| macOS | `.dmg`, `.app` |
| Windows | `.msi`, `.nsis` |

## Project Structure

```
shekyl-gui-wallet/
  .github/workflows/       # CI and release pipelines
  src/                      # React frontend
    components/             # Sidebar, Header, BalanceCard, NetworkBadge,
                            # ChainHealthPanel, EmissionGauge, StakeTierCard,
                            # PqcStatusBadge, TestnetBanner
      __tests__/            # Component unit tests
    pages/                  # Dashboard, Send, Receive, Staking, Transactions,
                            # Settings, ChainHealth
      __tests__/            # Page unit tests
    context/                # DaemonProvider (30s polling), useDaemon hook
    types/daemon.ts         # Shared TypeScript interfaces for daemon RPC
    lib/format.ts           # SKL formatting utilities (9-decimal atomic)
    test/setup.ts           # Vitest global setup (RTL matchers, Tauri IPC mock)
    styles/globals.css      # Tailwind CSS with Shekyl color palette
  src-tauri/                # Rust backend
    src/
      lib.rs                # Tauri app builder, state management, command registration
      commands.rs           # Daemon-connected + wallet stub commands + unit tests
      daemon_rpc.rs         # Async JSON-RPC client for shekyld
      state.rs              # App state (daemon URL, network, HTTP client)
      main.rs               # Entry point
    tauri.conf.json         # App metadata, window config, bundle settings
    capabilities/           # Tauri 2 permission system
    rustfmt.toml            # Rust formatting rules
  public/assets/            # Branding SVGs
  vitest.config.ts          # Vitest configuration
  docs/
    CHANGELOG.md            # Wallet-specific changelog
    INSTALLATION.md         # End-user installation guide
```

## License

BSD-3-Clause. See [LICENSE](LICENSE) for details.

Copyright (c) 2026, The Shekyl Foundation.
