# Shekyl GUI Wallet

Cross-platform desktop wallet for Shekyl (SKL), built with **Tauri 2**, **React**, and **Rust**.

> **New to Shekyl?** Read the [User Guide](docs/USER_GUIDE.md) for a
> beginner-friendly walkthrough of every feature.

## Features

- **FCMP++ privacy** — full-chain membership proofs replace ring signatures,
  giving every transaction an anonymity set equal to the entire UTXO set;
  live curve tree statistics shown in Chain Health and the Security Panel
- **3-layer security model** — membership proof (FCMP++), hybrid spend
  authorization (Ed25519 + ML-DSA-65), and amount privacy (Bulletproofs+);
  compact SecurityBadge in the header with detailed SecurityPanel in Settings
- **Real-time proof progress** — FCMP++ proof generation, PQC signing, and
  broadcast stages streamed from the C++ wallet2 core through the FFI and
  Tauri event system — no cosmetic timers
- **Live chain health dashboard** — emission era, release tempo, burn rate,
  stake ratio, total burned/staked, hash rate, block reward, anonymity set
  size, curve tree depth, and tree root from the daemon
- **Per-wallet staking** — stake and claim rewards directly from the wallet;
  tier selection (Tier 0/1/2) with live APY estimates, accrual pool
  commingling explanation, and plausible deniability on claims
- **In-wallet mining** — start/stop mining from the GUI with thread control,
  background mining toggle, live hash rate gauge, and privacy note about
  60-block coinbase maturity
- **Help Center** — expandable guide sections covering Getting Started,
  Mining, Staking, Security and Privacy (FCMP++/PQC), Multisig, and a
  Glossary of key terms
- **Network switching** — MainNet / TestNet / StageNet with automatic daemon
  URL configuration, persistent testnet banners, and faucet links
- **9-decimal canonical SKL display** — 6-decimal readability with full
  precision available
- Create, open, import, and manage Shekyl wallets via wallet2 C++ FFI
- Send and receive SKL with real progress feedback
- Transaction history with fee, timestamp, and confirmation status
- Modern dark UI with the Shekyl gold & purple design system

## Architecture

```
Frontend:  Vite + React 19 + TypeScript + Tailwind CSS 4
Backend:   Rust (Tauri 2) — JSON-RPC client to shekyld daemon
                           + C++ FFI bridge to wallet2 (shekyl-core)
IPC:       Tauri invoke bridge + Tauri events (progress callbacks)
TLS:       rustls (no OpenSSL dependency)
```

The Rust backend connects to a `shekyld` daemon via JSON-RPC for chain health,
staking, and economic data, and via plain HTTP endpoints for mining control.
Wallet-side operations (create, transfer, stake, claim, import) are handled by
the `shekyl-wallet-rpc` crate which wraps the wallet2 C++ core through a safe
FFI boundary. Progress events (FCMP++ proof generation, PQC signing, broadcast)
flow from C++ through an `mpsc` channel to the Tauri event system.

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

### Version management

The version is defined once in `package.json` and synced everywhere:

```bash
# Bump and sync (two steps)
npm run version:bump -- 0.2.0-beta   # updates package.json
npm run version:sync                  # propagates to Cargo.toml + tauri.conf.json
```

The frontend reads `__APP_VERSION__` at build time via Vite's `define`, so
`Sidebar.tsx` never needs a manual version edit.

## Testing

### Frontend (Vitest + React Testing Library)

```bash
npm test                # Single run (65 tests, 13 suites)
npm run test:watch      # Watch mode
npm run test:coverage   # With V8 coverage report
```

Tests live alongside the code in `__tests__/` directories. Tauri IPC calls
are mocked via `@tauri-apps/api/mocks` so tests run without a Rust backend.

### Rust backend (cargo test)

```bash
cd src-tauri
cargo test              # 10 tests
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
                            # SecurityBadge, SecurityPanel, TestnetBanner
      __tests__/            # Component unit tests
    pages/                  # Dashboard, Send, Receive, Mining, Staking,
                            # Transactions, Settings, ChainHealth, Help,
                            # ImportWallet
      __tests__/            # Page unit tests
    context/                # DaemonProvider (30s polling), useDaemon hook
    types/daemon.ts         # Shared TypeScript interfaces for daemon RPC
    lib/format.ts           # SKL formatting utilities (9-decimal atomic)
    test/setup.ts           # Vitest global setup (RTL matchers, Tauri IPC mock)
    vite-env.d.ts           # Global type declarations (__APP_VERSION__)
    styles/globals.css      # Tailwind CSS with Shekyl color palette
  scripts/
    sync-version.mjs        # Propagates package.json version → Cargo.toml, tauri.conf.json
  src-tauri/                # Rust backend
    src/
      lib.rs                # Tauri app builder, state management, command registration
      commands.rs           # Daemon + mining + wallet + staking + security commands
      daemon_rpc.rs         # JSON-RPC + HTTP client for shekyld (chain, mining, curve tree)
      wallet_bridge.rs      # wallet2 FFI bridge, progress event pipeline, staking
      state.rs              # App state (daemon URL, network, HTTP client)
      main.rs               # Entry point
    tauri.conf.json         # App metadata, window config, bundle settings
    capabilities/           # Tauri 2 permission system
    rustfmt.toml            # Rust formatting rules
  public/assets/            # Branding SVGs
  vitest.config.ts          # Vitest configuration
  docs/
    USER_GUIDE.md           # Beginner-friendly walkthrough of every feature
    CHANGELOG.md            # Wallet-specific changelog
    INSTALLATION.md         # End-user installation guide
```

## License

BSD-3-Clause. See [LICENSE](LICENSE) for details.

Copyright (c) 2026, The Shekyl Foundation.
