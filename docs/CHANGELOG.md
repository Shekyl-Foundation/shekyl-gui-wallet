# Shekyl GUI Wallet Changelog

## [Unreleased]

### Added

- **"Your stake" panel on the Staking page (GUI-PR3b).** Active stakers now
  see their staked balance and outputs, projected from the core
  `Engine::staking_read_view` (WI-RPC-1) — the one authoritative aggregation
  over the sealed persona-scan / pending-post records. The three balance legs
  render as distinct figures, never summed (rule 82): **Bonded (confirmed)**,
  **Bonded (pending)** (sealed posts not yet on chain), and **Rewards
  (unspent)**. Each unspent staked output lists its slot, amount, and unlock
  height, with the persona-scan sync frontier below. Tauri command
  `get_staking_view` fails closed like the core read: a corrupt or
  version-mismatched staking seal is an explicit fault message, never an
  empty "nothing staked" panel. Deleted with it: the claim-era
  `get_staking_info` placeholder (fabricated empty list) and the three
  staked-output scanner stubs (`get_scanner_staked_outputs` /
  `get_scanner_claimable_stakes` / `get_scanner_unstakeable_outputs`) —
  `get_staking_view` is their Engine-native replacement.

### Changed

- **Adapted to shekyl-core send-journal/ledger drift (PR-SJ-1b, PR-SJ-3).**
  Balance now reads through core `WalletLedgerExt::balance` (journal-composed:
  an in-flight send counts in total, never unlocked); incoming-row pending
  status derives from the journal's in-flight spend locks (the persisted
  ledger field was retired upstream); and the Transactions list gains a
  distinct **Abandoned** status for sends the user told the wallet to stop
  tracking (never collapsed into Dropped; a late confirmation still flips it
  to Confirmed).

- **Sent transactions appear in Transactions history (PR-SJ-2 GUI
  enablement).** `transfer_history` merges the Engine send journal with
  receive-ledger rows so outgoing payments show with realized fee and a
  distinct status per lifecycle arm — **Pending**, **Confirmed**,
  **Failed** (daemon refused; never mined), **Dropped** (wallet stopped
  waiting; funds spendable again), plus receive-side **Spent**. Arms never
  collapse (rule 82). Projection mirrors wallet-rpc PR-SJ-2 (one row per
  receive output, same order key, typed status; newest-first for the UI;
  inclusion height absent until on chain). Closes the GUI half of the
  send-journal W-D surface landed in `shekyl-core` PR-SJ-1/#414 +
  PR-SJ-2/#420. The Transactions page polls every 15s (and on window focus)
  so status advances without remount, surfaces load failures with a retry
  action instead of an empty list (rule 82), and unit-tests the arms.

- **Drainable (P) balance on the Staking page (DS-PR-3 PR-B;
  `ARCHIVAL_DRAIN_SEND_FD2.md` §1).** The active-staker panel now shows the
  aggregate spendable `P` figure a drain could send, read from the core
  `Engine::drain_balance_aggregate` accessor (DS-PR-3 PR-A) — anchored to the
  same send-path reference a real drain proves against, not raw tip. Tauri
  command `get_drain_balance` returns a two-shape result that keeps the core
  distinction alive across the boundary (rule 82): a transient anchor-lag renders
  **"Syncing…"** (never a zero), and a non-transient read fault renders **"—"**
  (never a fabricated zero) — only a genuine `ready` result shows an SKL value.
  Aggregate-only by construction: no reward decomposition crosses the surface
  (F-D1 trust boundary). Requires the DS-PR-3 PR-A engine-core accessor.

- **Staker activation (GUI-PR3).** Staking page can activate an archival
  staker on the Engine backend: password re-auth → optional first-stake
  intent reopen → `Engine::first_stake`. Bond post is sealed as
  `pending_dispatch` (not broadcast on this call). Tauri commands:
  `activate_staker`, `get_staker_status`. Errors map funding / in-flight /
  already-staked refusals to clear UI text.

- **Engine send lifecycle (GUI-PR2).** `transfer` on the Engine backend builds
  and submits a pending tx (`build_pending_tx_async` → `submit_pending_tx_async`,
  `FeePriority::Standard`). CT-5d `ContentChanged` is resubmitted once with the
  advanced `content_gen`. `estimate_fee` builds then discards a pending tx for a
  real fee. `get_transactions` projects ledger receive outputs as incoming
  rows grouped by transaction.

- **Pure-Rust Engine wallet session (GUI-PR1).** New `engine_session` module
  embeds `shekyl-engine-core::Engine` directly (create / open / close /
  refresh / balance / primary address / BIP-39 restore). Tauri command:
  `refresh_wallet`. Engine files use `{name}.wallet` + `{name}.wallet.keys`.
  Create returns a BIP-39 mnemonic on mainnet/stagenet (raw hex on testnet).
  Mid-session seed display is intentionally unavailable (seed dropped at open).

### Fixed

- **The wallet-startup failure message no longer tells users to install
  something that does not exist.** On an `init_wallet_rpc` failure the UI said
  "Make sure shekyl-engine-rpc is installed and accessible" — naming a crate
  that has been deleted from `shekyl-core`, and implying a separate installable
  wallet service. There is none: the wallet runs in-process as
  `shekyl-engine-core::Engine`, and that command's only failure mode is
  preparing the wallet directory (permissions, or a path that exists as a
  file). The message now says the wallet is part of the app and points at the
  actionable remedy — choose a different wallet folder in Settings (rule 82).
  The specific cause continues to be surfaced verbatim from the backend, which
  already returns path-free, cause-specific strings.

### Changed

- **Documentation retired alongside the `shekyl-engine-rpc` deletion in
  `shekyl-core`.** The architecture docs still described `wallet_bridge.rs` and
  a C++ `wallet2` FFI backend as *current*, two migrations after both were
  deleted (GUI-PR1 moved the wallet onto the in-process Engine; `shekyl-core`
  then deleted the crate). Corrected to the real shape — `engine_session.rs`
  embedding `shekyl-engine-core::Engine`, refresh via `Engine::start_refresh`,
  scan state owned by the Engine rather than a GUI-held mutex — across
  `README.md`, `CONTRIBUTING.md`, `docs/WALLET_STARTUP.md` (architecture,
  open/close flow, concurrency model), `docs/GUI_SECURITY.md`, and
  `src-tauri/binaries/README.md`. Two `FOLLOWUPS.md` entries are closed as done
  (the `WALLET_REWRITE_PLAN.md` umbrella C++-dependency deletion target, and the
  `STAGE_1_PR_4` "GUI's local sync loop is replaced" item) and the multisig
  Cargo-feature gap is restated, since that feature named no code even when the
  dep existed. `BIP39_GUI_PREP.md`'s integration checklist is marked superseded:
  it routed through `wallet2_ffi` symbols that `shekyl-core` declined to add.
  `.gitignore` drops the `shekyl-engine-rpc-*` sidecar pattern for a binary that
  can no longer exist. CHANGELOG history is left as written.

- **CI now runs the backend unit tests.** The `ci.yml` step that previously
  only compiled the Rust tests (`cargo check --tests`, executing nothing) now
  runs them (`cargo test`) — all backend unit tests execute on every push/PR.
  Sidecar-dependent integration tests are marked `#[ignore]` (none exist yet)
  and run only in the release build (`release.yml`) after the real `shekyld`
  sidecar is compiled, via `cargo test --release -- --ignored`. Convention
  documented in `CONTRIBUTING.md`.

- **Engine is the sole wallet backend.** The transitional Wallet2 /
  `shekyl-engine-rpc` path and the `SHEKYL_ENGINE_BACKEND` flag are removed;
  `wallet_bridge` and the `get_engine_backend` / `set_engine_backend` commands
  are gone. Every wallet lifecycle and money command now runs only on the
  Engine. Features that lived solely on Wallet2 (import-from-keys, PQC
  multisig, scanner freeze/thaw + `get_scanner_*`) stay registered but return
  an honest "not available on the Engine backend" error until they are ported.
  Retired claim-era dead code (`stake` / `claim_rewards` commands,
  `validate_tier`) is deleted.

- **Transaction history projects send-journal outgoing rows** (see Added
  above). Receive-side rows are one output each (change included as
  incoming); spent outputs are labeled Spent, not re-projected as fabricated
  outgoing debits. Never-mined / unsettled rows sort to the top of the
  newest-first list.

- **Staking honesty mode (GUI-PR0).** Claim-era tier lock / claim-rewards UX is
  removed from the Staking page. The page now explains archival staking
  (activate → fund persona → hold shards → later unbond/drain), shows
  network-wide daemon stats only, and (with GUI-PR3) offers staker activation
  on the Engine backend while funding / unbond / drain remain pending.
  `get_balance` no longer reports a claim-era staked total; the Engine does
  not yet compute a personal `staked` total (Stage 3), so it reads zero as an
  honest "not yet available". Help center and `USER_GUIDE.md` restated to
  match. **shekyl-core pin:** `cf375a786` (dev tip, 2026-07-18 — stake
  activation entry PR #336). Product default: principal-focused desktop UX
  (not full operator node in-app).

- **BIP-39 prep (GUI only).** User-facing copy, import validation, and docs now
  describe a **24-word recovery phrase** (BIP-39 English) instead of a
  25-word legacy seed. Import rejects 25-word phrases client-side with a clear
  message. Optional passphrase field on import (UI only; wired in the
  follow-up integration PR). Info banners note that full create/restore requires
  an updated Shekyl core build.

### Note

- Functional mainnet create and BIP-39 restore are **not** complete until
  shekyl-core ships `wallet2_ffi_create_wallet_from_bip39` and BIP-39 restore
  FFI and the gui-wallet integration PR lands. See
  `docs/design/BIP39_GUI_PREP.md`.
- Archival staker activation / funding / drain require the Engine backend
  (GUI-PR1+) and are intentionally not faked in this release.

## [3.1.0-alpha.5] - 2026-05-19

> Resync against `shekyl-core/dev` after the April 26 scanner-state
> migration. alpha.4 does **not** compile against current
> `shekyl-core/dev` (`shekyl_scanner::WalletState` /
> `shekyl_scanner::sync::run_sync_loop` were retired upstream); alpha.5
> swaps the GUI's scanner bridge to the new `(LedgerBlock,
> LedgerIndexes)` shape, broadens filename sanitization to be the
> single source of truth, and persists the custom wallet directory
> across launches. The bundled `shekyld` carries upstream's DAA LWMA-1
> Phase 4 and RandomX v2 Phase 1 wiring; neither changes the daemon
> JSON-RPC contract the GUI reads.
>
> **shekyl-core pinned at `fe0457737`** (dev tip, 2026-05-19). The
> `release.yml` workflow still tracks `dev` per the alpha-cadence
> decision; pinning by tag is deferred to beta cadence (see
> `FOLLOWUPS.md`).

### Fixed

- **Build against current `shekyl-core/dev`.** `shekyl-scanner` retired
  `WalletState` and `sync::run_sync_loop` in upstream commit
  `252d942d2` (2026-04-26). The GUI's `wallet_bridge.rs` was the only
  consumer, and continued referencing the removed surface — alpha.4
  no longer compiles against current core. alpha.5 migrates to the
  forward shape (`(LedgerBlock, LedgerIndexes)` tuple per
  `shekyl-engine-rpc::ScannerState::LiveLedger`) and replaces
  `run_sync_loop` with a thin in-process loop in `wallet_bridge.rs`.
  Behavioural contract unchanged: same daemon polling cadence (5s
  when at-tip), same reorg detection (parent-hash compare → fork
  walk → `LedgerIndexes::handle_reorg`), same `scanner-progress`
  Tauri event after each block.

### Changed

- **Scanner bridge migrated to `(LedgerBlock, LedgerIndexes)`.**
  `WalletBridge.scanner_state` is now
  `Arc<TokioMutex<(LedgerBlock, LedgerIndexes)>>` — the same shape
  core uses internally. All scanner-backed query commands
  (`get_scanner_balance`, `get_scanner_staked_outputs`,
  `get_scanner_claimable_stakes`, `get_scanner_unstakeable_outputs`,
  `get_scanner_height`, `scanner_freeze`, `scanner_thaw`) updated to
  destructure the tuple and use the methods that landed on
  `LedgerBlock` (read-only queries) and `LedgerIndexes`
  (mutating-and-spend ops). `scanner_freeze` / `scanner_thaw` now
  route through `LedgerIndexes::{freeze,thaw}_by_key_image`, which
  hold the indexes side immutably and mutate the ledger.

- **`wallet_name::sanitize` is now the single source of truth for
  filename policy.** Previously sanitize only handled whitespace and
  `validate_wallet_name` separately rejected path separators, null
  bytes, and leading dots. The split led to two policy authorities
  that could (and did, in cross-platform edge cases) disagree.
  alpha.5 broadens `sanitize` to the full filesystem-safe
  transformation:

  - Any character outside `[A-Za-z0-9_\-.]` plus Unicode letters is
    replaced with `_`.
  - Runs of `_` (originally present or produced by the replacement
    pass) collapse to a single `_`.
  - Leading / trailing `_`, `.`, and whitespace are trimmed (so
    `..hidden` → `hidden`, `wallet.` → `wallet`).

  `validate_wallet_name` now only checks "not empty" (catches the
  all-forbidden-chars input → empty sanitize output case) and
  "under `MAX_WALLET_NAME_LEN`". Path-policy tests moved from
  `validate.rs` to `wallet_name.rs`; a new proptest
  (`sanitize_output_is_filesystem_safe`) asserts the post-sanitize
  alphabet across arbitrary `\PC{0,500}` inputs. The dual-search
  fallback on `open_wallet` is retained for one more release so
  pre-alpha.5 wallets keep opening; see `FOLLOWUPS.md`.

- **`get_wallet_dir` returns `{ dir, fallback_from? }` instead of a
  bare string.** When the persisted custom wallet directory is
  unreachable at startup (permission denied, target-is-a-file,
  symlink to a missing target), the app silently falls back to the
  platform default and surfaces the original path in
  `fallback_from`. The Advanced disclosure renders an amber warning
  banner using this field; explicit `set_wallet_dir` or
  `reset_wallet_dir` clears it. The TypeScript context exposes
  `walletDirFallbackFrom: string | null` for component consumers.

### Added

- **Persistent custom wallet directory** via
  `gui-config.json`. Stored under the platform's per-app config
  directory (`~/.config/org.shekyl.wallet/gui-config.json` on Linux,
  `~/Library/Application Support/org.shekyl.wallet/gui-config.json`
  on macOS, `%APPDATA%\org.shekyl.wallet\gui-config.json` on
  Windows), the file persists the user's chosen wallet directory
  across launches. Failures are silent and recoverable: a missing,
  malformed, or future-schema file falls back to the platform
  default without error. Writes are atomic
  (`*.tmp` + `rename`), best-effort, and logged at `warn!` on
  failure. Schema is `{ "schema_version": 1, "wallet_dir_override":
  "..." | null }` — additive future fields use `#[serde(default)]`.

- **`gui_config` Rust module** (`src-tauri/src/gui_config.rs`)
  exposes `load()`, `save(&GuiConfig)`, and
  `resolve_wallet_dir(default_dir) -> ResolvedWalletDir`. Tests use
  path-injected variants (`load_from_path`, `save_to_path`,
  `resolve_wallet_dir_in`) so they don't touch the developer's real
  config dir and don't mutate process env vars (deprecation-warned
  in Rust 1.83+).

- **Bundled `shekyld` carries shekyl-core's May-window consensus
  work** (no GUI behaviour change, listed here for traceability):

  - **DAA LWMA-1 Phase 4** (atomic cutover landed in core
    `PR #53`). New mainnet difficulty algorithm targeting 120-second
    blocks. The `mining_status.block_target` and `get_info.target`
    RPC fields are pinned to `120`; `daemon_rpc.rs` already models
    those fields correctly, so the GUI displays the new target
    without code changes.
  - **RandomX v2 Phase 1** (build-wiring only, core `PR #54`). The
    bundled daemon now compiles against the RandomX v2 source tree
    via a Git submodule; no consensus change yet. `release.yml`
    already passes `--recurse-submodules`; local developer
    workflows pulling against a sibling `../shekyl-core` clone
    need a one-time
    `git submodule update --init --recursive`.

### Build notes

- New direct dependencies on shekyl-core path crates:
  `shekyl-rpc`, `shekyl-oxide`, `shekyl-crypto-pq`. These are
  required for the in-process sync loop (`Rpc` trait, `Input`
  enum, `KeyImage` wrapper); previously the scanner re-exports
  carried them transitively. Adding direct deps surfaces the
  workspace path in `Cargo.toml` review.
- `shekyl-scanner` no longer needs its `rust-scanner` feature flag
  (the scanner crate retired the feature in core commit
  `252d942d2`). The `rust-scanner` feature on **`shekyl-engine-rpc`**
  survives because it still gates the scanner-crate import; the GUI
  enables it on `shekyl-engine-rpc` only.
- `thiserror = "1"` added as a direct dep (already transitive) for
  the local `SyncError` enum in `wallet_bridge.rs`.

## [3.1.0-alpha.4] - 2026-04-18

> **alpha.3 is broken on Windows.** Wallet creation produced a mixed-
> separator path (`C:\Users\…\Shekyl/My Wallet.keys`) that Windows
> refused, so alpha.3 could not create or open wallets on Windows at
> all. alpha.4 fixes this; anyone who tried alpha.3 on Windows should
> upgrade directly and discard any partial wallet directory.

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

- **Dependency-Audit CI workflow now runs on `dev`.** The scaffolded
  trigger referenced a `develop` branch that has never existed in this
  repo, so direct pushes to `dev` silently skipped the audit. The lag
  showed up concretely in the alpha.2 → alpha.3 cycle: RustSec indexed
  RUSTSEC-2026-0098 and -0099 within hours of tagging alpha.2 and we
  only caught it on the next scheduled run. Retargeting the workflow
  closes that window.

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
