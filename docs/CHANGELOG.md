# Shekyl GUI Wallet Changelog

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
