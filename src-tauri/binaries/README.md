# Sidecar Binaries

This directory holds platform-specific binaries that are bundled into the
Shekyl Wallet installer via Tauri's `externalBin` feature.

## Bundled binary

| Binary    | Purpose                  | Bundled since |
|-----------|--------------------------|---------------|
| `shekyld` | Shekyl blockchain daemon | alpha.2       |

`shekyl-wallet-rpc` is **not** bundled — the GUI wallet imports it as a
Rust library dependency, so no separate binary is needed.

## How it works

The release CI workflow (`.github/workflows/release.yml`) builds `shekyld`
from shekyl-core on all three platforms and copies the binary into this
directory with the required Tauri triple-suffix naming:

```
shekyld-x86_64-unknown-linux-gnu
shekyld-aarch64-apple-darwin
shekyld-x86_64-pc-windows-msvc.exe
```

`tauri.conf.json` declares `"externalBin": ["binaries/shekyld"]`, so
Tauri automatically selects the matching triple at build time and bundles
it into the installer.

## Naming convention

Tauri requires target-triple suffixed names. You can check your local
target triple with:

```bash
rustc -vV | grep host
```

Only the platform you build for needs the binary present.

## Lifecycle

The `DaemonManager` in `src-tauri/src/daemon_manager.rs` handles the
sidecar lifecycle:

1. On app launch, checks if a daemon is already running on the RPC port.
2. If not, spawns the bundled `shekyld` sidecar with appropriate flags.
3. Polls `/get_info` until the daemon is ready (30s timeout).
4. On app exit, sends SIGTERM (or taskkill on Windows) unless the user
   has enabled "Keep daemon running after wallet closes" in Settings.

## Git

These binaries are large and should NOT be committed to the repository.
They are listed in `.gitignore`. CI produces them at build time.

## Development

During `tauri dev`, the sidecar binary is not present. The
`DaemonManager` detects this gracefully — it logs a warning, sets the
daemon mode to `Unavailable`, and the wallet falls back to whatever
daemon URL is configured in Settings. Run `shekyld` manually or point
to an existing node.
