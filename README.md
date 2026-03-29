# Shekyl GUI Wallet

Cross-platform desktop wallet for Shekyl (SKL), built with **Tauri 2**, **React**, and **Rust**.

## Features

- Create, open, and manage Shekyl wallets
- Send and receive SKL
- Staking with tier selection (Tier 0/1/2)
- Transaction history
- Daemon connection management (MainNet / TestNet / StageNet)
- Modern dark UI with the Shekyl gold & purple design system

## Architecture

```
Frontend:  Vite + React + TypeScript + Tailwind CSS
Backend:   Rust (Tauri 2 commands)
IPC:       Tauri invoke/listen bridge
Future:    C++ FFI bridge to wallet2_api.h (shekyl-core)
```

The Rust backend currently provides stub commands that return mock data.
Phase 2 will bridge to the Shekyl core wallet library via FFI, enabling
real wallet operations.

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
npm test                # Single run
npm run test:watch      # Watch mode
npm run test:coverage   # With V8 coverage report
```

Tests live alongside the code in `__tests__/` directories. Tauri IPC calls
are mocked via `@tauri-apps/api/mocks` so tests run without a Rust backend.

### Rust backend (cargo test)

```bash
cd src-tauri
cargo test
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

- **CI** (`.github/workflows/ci.yml`) -- runs on every push/PR to `main`:
  ESLint, TypeScript type-check, Vitest, Rustfmt, Clippy, and cargo test.
- **Release** (`.github/workflows/release.yml`) -- runs on push to the
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
  .github/workflows/     # CI and release pipelines
  src/                    # React frontend
    components/           # Sidebar, Header, BalanceCard, NetworkBadge
      __tests__/          # Component unit tests
    pages/                # Dashboard, Send, Receive, Staking, Transactions, Settings
      __tests__/          # Page unit tests
    test/setup.ts         # Vitest global setup (RTL matchers, Tauri IPC mock)
    styles/globals.css    # Tailwind CSS with Shekyl color palette
  src-tauri/              # Rust backend
    src/
      lib.rs              # Tauri app builder and command registration
      commands.rs         # Wallet command stubs + unit tests (Phase 2: real FFI)
      main.rs             # Entry point
    tauri.conf.json       # App metadata, window config, bundle settings
    capabilities/         # Tauri 2 permission system
    rustfmt.toml          # Rust formatting rules
  public/assets/          # Branding SVGs
  vitest.config.ts        # Vitest configuration
```

## License

BSD-3-Clause. See [LICENSE](LICENSE) for details.

Copyright (c) 2026, The Shekyl Foundation.
