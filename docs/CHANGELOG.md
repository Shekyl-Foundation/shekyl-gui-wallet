# Shekyl GUI Wallet Changelog

## [Unreleased]

### Fixed

- **Windows wallet-file path corruption.** On Windows, creating a wallet
  named "My Wallet" produced the path
  `C:\Users\<user>\...\Shekyl/My Wallet.keys` — correct directory
  separators for the profile prefix, a POSIX `/` for the join, and an
  unescaped space in the filename. Path construction has been moved
  from the C++ `wallet2_ffi` layer (which previously carried
  `wallet_dir` state and concatenated strings manually) into a new Rust
  module `src-tauri/src/wallet_name.rs` that uses `PathBuf::join`, so
  the host separator is always correct. User-supplied names are
  sanitized (`"My Wallet"` → `"My_Wallet"`) before ever touching disk.
  Addresses the alpha.3 field report; bugfix only, no consensus change.

### Added

- **`wallet_name` Rust module** (`src-tauri/src/wallet_name.rs`)
  centralising wallet-filename policy:
  - `sanitize()` — trims, collapses internal whitespace, replaces with
    underscores; idempotent.
  - `build_wallet_path()` — `PathBuf::join` wrapper so callers can't
    accidentally hand-concatenate separators.
  - `ensure_dir_exists()` — `mkdir -p` via `std::fs::create_dir_all`
    with a user-safe error message that does not echo the input path
    back (preserves the no-secret-leakage posture asserted in
    `validate` tests).
- **`validate::validate_wallet_path()`** — rejects empty paths, paths
  over `MAX_WALLET_PATH_LEN` (4096 bytes), paths containing null
  bytes, and paths with no filename component. Covered by proptest
  entry `validate_wallet_path_never_panics`.
- **Dual-search on `open_wallet`.** If the user types a wallet name
  with spaces, the backend first tries the sanitized
  (`"My_Wallet"`) filename and falls back to the raw
  (`"My Wallet"`) filename so pre-alpha wallets created before the
  sanitizer shipped still open. Tracked for deletion in
  `FOLLOWUPS.md` after one minor release.
- **Custom wallet directory.** New Tauri commands `set_wallet_dir`,
  `reset_wallet_dir`, `get_wallet_dir` and context methods
  `setCustomWalletDir`, `resetWalletDir`, `refreshWalletDir` let users
  move the wallet folder off the platform default (e.g. onto an
  encrypted volume). `set_wallet_dir` runs `mkdir -p` on the chosen
  path and refreshes the wallet-file list.
- **"Advanced: wallet file location" disclosure** on the Create,
  Import, and Unlock screens. Collapsed by default so the happy-path
  user never has to think about filesystem layout; advanced users
  open the disclosure to see the current directory and pick a
  different folder via the native `tauri-plugin-dialog` picker.
- **`tauri-plugin-dialog` v2** added to `src-tauri/Cargo.toml` and
  registered in `lib.rs`. Capability `default.json` gains
  `dialog:default`, `dialog:allow-open`, `dialog:allow-save` so the
  frontend can invoke `open({ directory: true })` without a custom
  permission grant.
- **Shared `CollapsibleSection` component** extracted from
  `pages/Help.tsx` to `components/CollapsibleSection.tsx`. Supports
  both controlled (Help, one-panel-at-a-time) and uncontrolled
  (Advanced disclosure) modes.

### Changed

- **`wallet_bridge::init` no longer takes `wallet_dir`.** Paired with
  the shekyl-core change that removes `wallet_dir` state from
  `wallet2_ffi`. The `filename` parameter on `create_wallet`,
  `open_wallet`, `restore_deterministic_wallet`, and
  `generate_from_keys` is now `wallet_path` — the Rust caller is
  responsible for joining, sanitizing, and validating.
- **`init_wallet_rpc` ensures the default wallet directory exists**
  on startup via `wallet_name::ensure_dir_exists`, matching the
  mkdir-p behaviour users expect from `monero-wallet-rpc --wallet-dir`.

## [3.1.0-alpha.3] - 2026-04-18

### Security

- **Bump `rustls-webpki` from `0.103.10` to `0.103.12`** (transitive via
  `rustls` → `reqwest` → `tauri`) to pick up fixes for two advisories
  disclosed 2026-04-14:
  - [RUSTSEC-2026-0098](https://rustsec.org/advisories/RUSTSEC-2026-0098):
    `rustls-webpki` accepted malformed X.509 name constraints that could
    allow a crafted certificate chain to bypass name-constraint
    verification.
  - [RUSTSEC-2026-0099](https://rustsec.org/advisories/RUSTSEC-2026-0099):
    denial-of-service via unbounded recursion during certificate path
    building.

  `rustls-webpki` is used only for outgoing HTTPS from the GUI shell
  (Tauri/reqwest), not by the bundled `shekyld` daemon or any on-chain
  verification path. Neither advisory is reachable by connecting to a
  malicious server the user did not initiate a request to, but "the PR
  isn't green" is a hard rule — alpha.2 shipped with a cargo-audit
  failure and is deleted accordingly.

### Changed

- **Deleted `v3.1.0-alpha.2` release and tag.** The alpha.2 release
  build produced working artifacts on Linux/macOS/Windows, but the
  corresponding `dev` branch audit went red within hours of the tag
  push when RustSec indexed the two `rustls-webpki` advisories. We
  don't ship binaries behind a known-red audit, even in alpha. Same
  pattern as the alpha.1–alpha.14 cleanup: if the tag isn't green, it
  doesn't stay tagged.
- `src-tauri/Cargo.toml`, `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.lock` all bumped to `3.1.0-alpha.3`.

## [3.1.0-alpha.2] - 2026-04-18 [DELETED]

> Tag and release deleted 2026-04-18 — see alpha.3 Security notes. The
> entry below is retained as engineering history for the version-reset
> event; it is not a shipped release.

### Changed

- **Version reset to match shekyl-core.** Prior alpha cycle (alpha.1
  through alpha.14) burned 14 tags debugging the Windows/vcpkg/Boost
  build chain; alpha.14 was the first to produce working artifacts on
  all three platforms (Linux, macOS, Windows). The `v3.1.0-alpha.1`
  through `v3.1.0-alpha.14` GitHub releases and tags have been deleted
  to align GUI wallet versioning with shekyl-core, which is now at
  `v3.1.0-alpha.2`. The changelog entries below are retained as
  engineering history — the vcpkg-decoration and environment-variable
  pitfalls are exactly the kind of thing a future maintainer hitting a
  similar problem will want to find.
- `src-tauri/Cargo.toml`, `package.json`, `src-tauri/tauri.conf.json`,
  `src-tauri/Cargo.lock` all updated to `3.1.0-alpha.2`.

### Added

- `docs/FOLLOWUPS.md` entry: pin `shekyl-core` checkout to the matching
  tag in `release.yml` before any release labeled reproducible. Target
  alpha.4. Current behavior (clone `shekyl-core` dev) is acceptable for
  alpha cadence but makes released `shekyld` binaries non-reproducible
  from the GUI wallet tag alone.

## [3.1.0-alpha.14] - 2026-04-17

### Fixed

- Windows build: match vcpkg's MSVC-decorated Boost filenames. The
  vcpkg x64-windows-static triplet installs Boost as
  `boost_system-vc145-mt-x64-1_90.lib` (toolset-threading-arch-version),
  not `boost_system.lib`. alpha.13 diagnostics confirmed the 48-file
  vcpkg lib listing: OpenSSL/sodium are plain (`libssl.lib`,
  `libcrypto.lib`, `libsodium.lib`) but Boost is decorated.
  `src-tauri/build.rs` now scans the vcpkg lib directory and accepts
  either the plain `{name}.lib` or any decorated `{name}-*.lib`,
  passing the discovered stem verbatim to rustc.
- Removed `protobuf` from the Windows static link list. It is only
  used by the Trezor backend, which is disabled on Windows
  (`-DUSE_DEVICE_TREZOR=OFF`), and is not installed via vcpkg.

## [3.1.0-alpha.13] - 2026-04-17

### Fixed

- Windows release workflow: actually propagate `VCPKG_INSTALLATION_ROOT`
  into later steps. alpha.11/12 tried to pass it via
  `${{ env.VCPKG_INSTALLATION_ROOT }}` in the tauri-action step, but
  that expression resolves against the workflow's `env` context (empty
  for this var) rather than the runner's OS environment, so the
  variable was silently overwritten with an empty string. The vcpkg
  search path then became `/installed/x64-windows-static/lib`, which
  does not exist, and every vcpkg static library was skipped. Fix:
  the "Install Windows build dependencies" step now writes
  `VCPKG_INSTALLATION_ROOT=$env:VCPKG_INSTALLATION_ROOT` into
  `$GITHUB_ENV` so subsequent steps (and their `env:` blocks) see
  the real value.

## [3.1.0-alpha.12] - 2026-04-17

### Fixed

- Windows build: handle alternate vcpkg library naming for OpenSSL,
  libsodium, and protobuf. `src-tauri/build.rs` now probes both the
  `lib`-prefixed and bare forms (`libssl`/`ssl`, `libcrypto`/`crypto`,
  `sodium`/`libsodium`, `libprotobuf`/`protobuf`) and links whichever
  is present in the vcpkg lib directory. alpha.11 fixed Boost but
  failed at the next step because vcpkg 1.90's OpenSSL port uses
  different names than were hard-coded.
- `src-tauri/build.rs`: dump the vcpkg lib directory contents as
  `cargo:warning` lines so future naming mismatches are diagnosable
  from CI logs without guesswork.

## [3.1.0-alpha.11] - 2026-04-17

### Fixed

- Windows build: pass `VCPKG_INSTALLATION_ROOT` through to the Tauri build
  step so the Rust linker can find vcpkg-installed Boost static libraries.
  The C++ daemon build already discovered Boost via the vcpkg toolchain
  file, but `src-tauri/build.rs` did its own linking and failed with
  `could not find native static library boost_system` when the env var
  was not inherited.
- `src-tauri/build.rs`: emit an explicit `cargo:warning` when
  `VCPKG_INSTALLATION_ROOT` is unset on Windows, and skip Boost libraries
  whose `.lib` file does not exist at the expected vcpkg path (handles
  header-only components gracefully).

## [3.1.0-alpha.10] - 2026-04-17

### Fixed

- shekyl-core: broke circular include dependency in daemon headers
  (`core.h`, `p2p.h`, `rpc.h`) by moving constructor/destructor bodies
  to `.cpp` files, eliminating the Windows daemonizer include chain that
  caused persistent MSVC C2039/C2061 errors.
- shekyl-core: removed the entire `daemonizer/` layer (Windows service,
  POSIX fork). `shekyld` and `shekyl-wallet-rpc` now run in foreground
  only, with OS service managers handling backgrounding.
- shekyl-core: fixed `ARCH_ID` case mismatch that prevented RandomX JIT
  compilation on MSVC (uppercase `AMD64` vs lowercase `amd64` check).
- shekyl-core: fixed C/C++ linkage mismatch in `blocks.cpp` for
  generated `.c` symbols (missing `extern "C"`).
- shekyl-core: added missing `#include <windows.h>` to `math_helper.h`
  for `FILETIME` type (exposed by GCC 15 `-Wtemplate-body`).

## [3.1.0-alpha.9] - 2026-04-16

### Fixed

- Made all shekyl-core daemon headers self-contained for MSVC: added
  missing includes to `protocol.h` (6), `p2p.h` (2), `daemon.h` (2),
  and `rpc.h` (2). This batch fix resolves the remaining MSVC C2061 /
  C2065 errors that surfaced one-at-a-time across alphas 5-8.

### Changed

- shekyl-core MSVC CI now builds `--target daemon wallet` (was
  `--target wallet` only), matching this repo's release workflow. Future
  MSVC regressions will be caught in shekyl-core CI, not here.

## [3.1.0-alpha.8] - 2026-04-16

### Fixed

- Fixed MSVC build error in daemon's `rpc.h`: the header used `t_core`,
  `t_p2p`, and `daemon_args` without including the headers that define
  them. GCC/Clang tolerated this because the includer (`daemon.cpp`) had
  already pulled them in; MSVC does not (C2061, C2065). Added explicit
  includes for `core.h`, `p2p.h`, and `command_line_args.h`.

## [3.1.0-alpha.7] - 2026-04-16

### Fixed

- Fixed two more MSVC build errors surfaced by `--target daemon` pulling
  in the RPC server:
  - `core_rpc_server.cpp`: replaced `#ifdef` inside `MERROR()` macro
    argument (undefined behavior, C2059 on MSVC) with a literal function
    name string.
  - `abstract_tcp_server2.inl`: explicitly capture `handshake` in lambda
    (C3493 on MSVC, same constexpr implicit-capture class as alpha.5).

## [3.1.0-alpha.6] - 2026-04-16

### Changed

- Release workflow: macOS and Windows CMake builds now use
  `--target daemon wallet` instead of building the entire tree. Only the
  sidecar binary and wallet libraries are needed; skipping debug utilities,
  blockchain utilities, etc. saves build time and avoids compiling
  unrelated code that may have platform-specific issues.

## [3.1.0-alpha.5] - 2026-04-16

### Fixed

- Implemented missing `wallet2_ffi_get_scanner_keys` C++ FFI function in
  shekyl-core. The function was declared in the Rust FFI bindings and called
  by the GUI wallet's sync loop but had no C++ implementation, causing
  `Undefined symbols` linker errors on all platforms.
- Fixed three MSVC compilation errors in shekyl-core that blocked the
  Windows release build:
  - C3493: explicit lambda capture of constexpr local in `core_rpc_server.cpp`
  - C2065: replaced `__PRETTY_FUNCTION__` with `__FUNCSIG__` on MSVC
  - C2039: SFINAE-constrained `network_address` template constructor to
    prevent MSVC from eagerly instantiating it with incompatible types

## [3.1.0-alpha.3] - 2026-04-16

### Added

- Bundled `shekyld` daemon as a Tauri sidecar. The installer now ships a
  complete node+wallet package; users no longer need to install `shekyld`
  separately.
- `DaemonManager`: auto-start daemon on wallet launch, health-check
  polling, graceful shutdown on exit. Detects external daemons and defers
  to them when already running.
- Advanced daemon setting: "Keep daemon running after wallet closes"
  toggle (default: off). Persisted to `{app_config_dir}/daemon.json`.
- Daemon status indicator on the Settings page (managed/external/offline).
- Tauri commands: `daemon_status`, `restart_daemon`, `get_daemon_settings`,
  `set_daemon_settings`.
- `capabilities/daemon.json` for shell sidecar permissions.
- Release CI builds `shekyld` on all three platforms (Linux, macOS,
  Windows) and places it for Tauri bundling.

### Fixed

- Release workflow: replaced deleted `--target wallet_api` CMake target
  with an unqualified build so Windows artifacts are actually produced.
- Linux release artifact upload: steps now fail loudly when `.deb` or
  `.AppImage` is missing, instead of silently succeeding with no upload.
- Fixed 10 lint errors in multisig scaffolding (unused imports, unused
  state setters, type mismatches between page state and component props).
- Replaced inline `Date.now()` calls in `FailureAlerts` render path with
  a `nowSecs` prop to satisfy the React purity lint rule.
- Installed missing `@tauri-apps/plugin-dialog` dependency (needed by
  `GroupDescriptor` file import/export).
- Fixed `rust_eh_personality` duplicate symbol linker error on Linux:
  replaced `rustc-link-lib=static=shekyl_ffi` (which bundles libstd
  inside the staticlib) with a normal Cargo dependency on `shekyl-ffi`
  (consumed as rlib, no second libstd).
- Fixed macOS sidecar copy path (`build/src/daemon/shekyld` ->
  `build/bin/shekyld`) to match CMake `RUNTIME_OUTPUT_DIRECTORY`.
- Fixed Windows sidecar copy path (`build\src\daemon\Release\shekyld.exe`
  -> `build\bin\Release\shekyld.exe`) for same reason.
- Removed unused `SHEKYL_SOURCE_DIR` env var from release workflow (no
  longer consumed after shekyl-ffi link strategy change).

### Changed

- Version bumped to 3.1.0-alpha.3 across all three version sources
  (package.json, src-tauri/Cargo.toml, tauri.conf.json).
- CI now runs `npm run build` (Vite production build) after typecheck to
  catch build regressions on every push.

## [3.1.0-alpha.1] - 2026-04-15

First release under the unified Shekyl versioning scheme. The version
jumps from 0.4.0-beta.2 to 3.1.0-alpha.1 to align the software major
version with the Shekyl protocol version at first public release. This
is not a regression from beta to alpha — it is a re-baseline. The
pre-release stage reflects that the combined system (daemon + wallet) has
not yet completed stressnet validation. See `shekyl-core/docs/VERSIONING.md`
for the full versioning scheme.

Software versions follow SemVer independently per repo. The GUI wallet
and shekyl-core are not version-coupled; each declares which protocol
version it requires. This release requires `protocol_version = 3`.

### Added

- V3.1 multisig UI components: FingerprintBadge, ProverView,
  LossAcknowledgment, AddressProvenance, RelayConfig, ViolationAlert,
  SigningDashboard.
- File-based transport for multisig signing (first-class, equal
  prominence with relay transport).
- GroupDescriptor export/import for multisig group backup.
- Failure-mode UX: 6 alert banners for multisig operational failures.
- Address format discipline rule (`65-address-format-discipline.mdc`).

### Changed

- Version scheme: aligned to Shekyl protocol versioning (3.x series).
  All three version sources (package.json, src-tauri/Cargo.toml,
  tauri.conf.json) now report 3.1.0-alpha.1.

## [0.4.0-beta.2] - 2026-04-13

_Last release under the pre-alignment version scheme._
