# Follow-ups

Items that don't fit the current release scope. Each has a target version.
Items without a target version get one within 30 days or get closed as
"won't fix." See `shekyl-core/docs/15-deletion-and-debt.mdc` for policy.

---

## Daemon transport has no proxy — target: with the desktop's remote-node story

The daemon client dials directly: `reqwest` is built without the `socks`
feature (`src-tauri/Cargo.toml`) and `engine_session.rs` constructs
`HttpRpc::new(url)` with no proxy, so a `.onion` or otherwise
proxy-reachable daemon address cannot be entered in **Settings**. The
troubleshooting guide therefore points at a local port forward, which
needs nothing from the wallet.

Two things would change that, and they are separable:

1. **A proxy setting — across *both* daemon transports.** This wallet
   dials the daemon two ways on the same configured URL: `HttpRpc`
   (`shekyl-rpc-transport`) for the Engine and scanner, and a plain
   `reqwest::Client` held in `AppState` for everything the UI polls —
   chain health, wallet status, staking info, curve-tree info, mining
   (`daemon_rpc::*`, ~12 call sites in `commands.rs`). Proxying only the
   first is worse than not starting: the wallet would scan through the
   proxy while every status panel reported the daemon disconnected.
   `shekyl-rpc-transport` already carries the SOCKS5h connector and the
   constructor for its half (`HttpRpc::with_proxy(url, proxy)` beside
   today's `HttpRpc::new(url)`); the `reqwest` half has no `socks`
   feature enabled and would need one, or — better, and the reason this
   is not a one-line item — the two transports collapse onto the one that
   already knows how to do this. Consolidation is the shape to aim at;
   a proxy field bolted onto a split transport is the shape to avoid.
2. **The §1 operator statement at the point of configuration.** The CLI
   and `shekyl-wallet-rpc` say what a non-loopback daemon's operator
   learns by serving (shekyl-core RT-W7,
   `shekyl_rpc_transport::network_posture::operator_warning`); this
   wallet configures the same address and says nothing. That call is one
   line — the panel needs a place to show it. Worth landing **before** a
   proxy field, not after: a proxy makes remote daemons easy to reach
   without making them any more the operator's own.

**Trigger:** either the first request for a remote daemon in the desktop
wallet, or the next Settings-panel change (item 2 alone). Note the
desktop scope is principal-focused, not an archival operator node, so a
built-in onion listener is *not* what this asks for.

---

## Archival staking + Engine backend — target: post-PR0 sequence

GUI-PR0 (this window) only makes staking **honest**: claim-era tier/claim UX
is gone; network stats remain. Real staking needs the Engine path.

**Locked product defaults (plan open-questions resolution):**

1. Desktop scope = **principal-focused** (activate, fund, later drain) — not
   a full archival operator node (onion HS / challenges) in-app.
2. Engine embedding = in-process wallet-rpc **or** direct `engine-core`.
3. Flip Engine default only after create/open/transfer parity.
4. Cold-start: light note + link to `STAKER_OPERATOR_GUIDE.md`.

**Sequence (do not reorder past honesty → Engine → activation):**

| PR | Work | Core gate | Status |
|----|------|-----------|--------|
| GUI-PR0 | Honesty-mode staking | — | **done** |
| GUI-PR1 | Engine session (create/open/close/refresh/balance) | Engine lifecycle on dev | **done** (Engine is the sole backend; Wallet2 path removed) |
| GUI-PR2 | Engine transfer (build+submit) + fee estimate + ledger history | same | **done** |
| GUI-PR3 | `activate_staker` / `stake { password }` + status + error map | PR #336 landed | **done** |
| GUI-PR3b | Staked-balance/outputs read panel (`staking_read_view`) | WI-RPC-1 on core dev | **done** |
| GUI-PR4 | `stake_in` funding UX | public/RPC `stake_in` (core PR-P3+) | next |
| GUI-PR5 | Multisig address-fingerprint cutover | group_id deleted in core | pending |
| GUI-PR6+ | unbond / drain / live shards | PR-P4/P5/P6 + emission | pending |

**GUI-PR3 leftovers:** activation without stake_in funding often returns
not-ready (expected until PR4). No UI for multi-slot W2 resume detail.
Next: GUI-PR4 `stake_in` funding.

**Deleted:** the Wallet2 backend, `wallet_bridge`, the `shekyl-ffi` /
`shekyl-engine-rpc` deps and the C++ static linkage, the
`SHEKYL_ENGINE_BACKEND` flag, claim-era `stake` / `claim_rewards` /
`validate_tier`, and (GUI-PR3b) the claim-era `get_staking_info`
placeholder plus the three staked-output scanner stubs
(`get_scanner_staked_outputs` / `get_scanner_claimable_stakes` /
`get_scanner_unstakeable_outputs`) — `get_staking_view` is their
Engine-native replacement. **Still to delete when done:** `StakeTierCard`
if unused, `get_tier_yields` if daemon tiers vanish, and the remaining
scanner stubs (`get_scanner_balance` / `get_scanner_height` /
`scanner_freeze` / `scanner_thaw`) once Engine-native equivalents exist.

---

## Per-stake views + unstake UI — item 1 landed (GUI-PR3b); item 2 target: GUI-PR6 window

The June 2026 design spike `feat/staking-views-ux` (archived as
`archive/feat/staking-views-ux-2026-08-09`, branch deleted) built a
per-stake "Your Stakes" list: maturity countdown (blocks + approx
duration), claimable-now reward, projected yield to maturity,
"Unlocked" / "Unstaking…" badges, and a per-stake Unstake action. It
was implemented on `shekyl_scanner::StakeView` /
`LedgerBlockExt::stake_views` (core branch `feat/scanner-stake-views`,
never landed) and the Wallet2 `wallet_bridge` path (retired at
GUI-PR1), with unstake riding the C++ wallet2 `json_rpc` path (also
retired).

Audited 2026-08-09: no equivalent has landed on dev — today's Staking
page has activation, staker status, and the drainable-P figure only —
but the branch is unrebasable; every layer it touches was replaced.
The archive tag is the design reference (UI layout, status semantics,
and its 5 Staking tests).

**Substrate correction (2026-08-09, same-day core audit).** The
Engine-native per-stake read **already exists** on core dev:
`Engine::staking_read_view()` (WI-RPC-1, `engine/staking_read.rs`)
returns `StakedBalance` (confirmed/pending bond principal,
rewards-received-unspent) plus per-output `StakedOutput` rows
(gindex, amount, p_slot, unlock_height, confirmed) and
`pscan_synced_height` — the same surface wallet-RPC's
`get_staked_balance` / `get_staked_outputs` / `staking_info` project
and the CLI consumes. Note its semantics are the archival-bond
model's, not the archived spike's claim-era ones: there is no
maturity countdown or yield projection (accrued-but-unclaimed
rewards are a named separate design item in core's FOLLOWUPS), and
pending-unbond state is durable in `PScanState` rather than
advisory. The core-side sibling branch (`feat/scanner-stake-views`,
archived as `archive/feat/scanner-stake-views-2026-08-09` in
shekyl-core) is superseded by this landed surface.

**Splits into two work items:**

1. **Staked-outputs read UI — LANDED (GUI-PR3b).**
   UPDATE 2026-08-09: the "Your stake" panel on the Staking page
   projects `Engine::staking_read_view()` through
   `engine_session.rs` / `get_staking_view`, with the three balance
   legs kept distinct and fail-closed read faults (rule 82). The
   staked-output scanner stubs and the claim-era `get_staking_info`
   placeholder were deleted in the same PR.
2. **Unstake/unbond action — still gated.** No unbond mutation is
   exposed on the Engine (`pub fn` audit 2026-08-09). Reimplement
   when core PR-P4 (unbond) exposes one to embedders, per the
   sequence table above.

Re-evaluation shape: fresh GUI PRs in the GUI-PR4+/PR6+ slots,
designed against the landed Engine API and using the archive tag as
the UX reference — not a revival of the archived commit.

---

## Multisig integration — target: alpha.3

Pre-release audit (2026-04-15) found the multisig UI is scaffolding that
has not been re-verified against the reworked shekyl-core V3.1 multisig
API. The UI components shipped in alpha.1 as visual scaffolding; wiring
them to the backend is blocked on the items below. Still blocked as of
alpha.5 — no shekyl-core side work landed in the alpha.4 → alpha.5
window.

### Backend gaps (shekyl-core side)

- **`sign_multisig_partial` is not implemented** in the C++ FFI dispatcher
  (`wallet2_ffi.cpp`). The entire signing flow is non-functional from the
  GUI.
- `get_pqc_multisig_info` returns only 4 fields (`is_multisig`, `n_total`,
  `m_required`, `group_id`). The UI expects additional fields
  (`fingerprint`, `participant_keys`, `spend_auth_version`) that never
  arrive.

### GUI wallet gaps

- ~~The `multisig` feature on `shekyl-engine-rpc` is not enabled in
  `src-tauri/Cargo.toml`, so the Rust FROST multisig handlers are not
  compiled into the wallet binary.~~ **Moot** — the dep was dropped at
  GUI-PR1 and the crate is now deleted from `shekyl-core`. Its `multisig`
  feature also named no code inside it (it only forwarded to
  `shekyl-engine-core` / `shekyl-fcmp`), so enabling it would never have
  compiled any handler. Restated: the GUI has **no** FROST multisig path
  today; wiring one means going through `shekyl-multisig` /
  `shekyl-engine-core`, not through a Cargo feature flag.
- `export_group_descriptor` produces mostly-empty data because
  `participant_pubkeys` and `address_fingerprint` come from JSON fields
  the backend doesn't return.
- The V3.1 dashboard (intents, violations, relay status, prover view) is
  pure scaffolding — state is initialized but never populated from backend
  commands that don't exist yet.

### Type mismatches — resolved in alpha.2

Three prop-type mismatches between multisig page state and the
components consuming it were pinned in alpha.2: `SigningDashboard`
now uses lowercase `IntentState` (`"proposed"` / `"signed"` / …)
instead of PascalCase, `ProverView` uses `amount: string` instead
of `number`, and `ViolationAlert` uses `invariantId: number` instead
of `string`. The TS types and component contracts now agree; see
`CHANGELOG.md` and the alpha.2 commit for the code. The lowercase
`IntentState` choice was made to match the component; the backend
wire format has not yet been specified, and step 5 of the plan below
is where the wire format gets confirmed against what the components
actually consume — if the backend returns PascalCase, the adapter
belongs at the Tauri command layer, not in the components.

### Plan

Create a dedicated multisig integration plan for alpha.3 that:

1. Implements `sign_multisig_partial` in `wallet2_ffi.cpp`.
2. Extends `get_pqc_multisig_info` to return all fields the UI needs.
3. Enables the `multisig` feature in the GUI wallet's Cargo.toml.
4. Wires the Rust FROST handlers to the Tauri command layer.
5. Connects the UI components to real backend data, confirming the
   wire format (`IntentState` casing, `amount`/`invariantId` types)
   matches what the components already consume, or introducing an
   adapter at the Tauri boundary if it doesn't.
6. Adds integration tests for the multisig signing round-trip.

---

## Pin shekyl-core by tag in release workflow — target: beta cadence

`.github/workflows/release.yml` currently clones `shekyl-core` from the
`dev` branch (and `ci.yml` / `codeql.yml` do the same):

```
git clone --depth 1 --branch dev --recurse-submodules \
  https://github.com/Shekyl-Foundation/shekyl-core.git ../shekyl-core
```

This is a reproducibility gap: replaying the GUI wallet `vX.Y.Z` build a
week later pulls whatever `dev` points at today, not what shipped. The
bundled `shekyld` binary in a release tarball therefore cannot be
reproduced from the git tag alone — it depends on when you run the
build.

**Status as of alpha.5:** explicitly deferred to beta cadence. The
alpha pipeline is still in motion on both sides (core's KeyEngine
migration, DAA LWMA-1 phase rollout, RandomX v2 phase work), so the
GUI continues cloning `shekyl-core/dev`. Each alpha changelog entry
records the specific dev SHA the bundled `shekyld` was built from so
the gap is at least auditable.

**Reversion criteria.** Per `shekyl-core/.cursor/rules/21-reversion-clause-discipline.mdc`,
the deferred-rejection here reopens to a positive decision when **any
one of**:

1. The GUI moves out of alpha cadence (first beta or RC tag).
2. `shekyl-core` publishes a `v3.1.0-alpha.N` tag matching a GUI
   alpha.N release window (and continues to do so).
3. An audit response or downstream user explicitly requests
   reproducible bundle builds for an alpha.

For alpha releases the gap is acceptable. Before tagging anything
users would hold long-term (beta, stable, or any release labeled
reproducible) the release workflow must pin to a specific
`shekyl-core` revision. Options, in order of preference:

1. **Matching-tag pin.** GUI wallet `v3.1.0-betaN` clones
   `shekyl-core` at tag `v3.1.0-betaN`. Requires that the shekyl-core
   tag exists before the GUI wallet tag is pushed. Makes the "which
   daemon ships in which wallet release" question trivially auditable.
2. **Pinned SHA.** The GUI wallet workflow reads a `SHEKYL_CORE_REV` file
   from its own repo and clones that exact commit. More flexible, less
   self-documenting, but decouples the two tag cadences.

CI (`ci.yml`) and CodeQL (`codeql.yml`) can stay on `dev` regardless
of release-workflow policy — they're "does current dev still build
against current dev" checks, which is exactly what we want there.

Before closing this item: verify the release workflow actually passes
the pinned tag to all three platforms' checkout commands, and update
`docs/BUILD.md` (or equivalent) to document the pin policy for anyone
building from source.

---

## Remove `open_wallet` dual-search fallback — target: alpha.6

`commands::open_wallet` currently tries the sanitized filename first
(`"My_Wallet.keys"`) and falls back to the raw filename
(`"My Wallet.keys"`) if the sanitized variant is not present on disk.
This exists only to rescue wallets created against alpha.1–alpha.3
builds, which wrote filenames with spaces intact on Windows and
consequently produced the separator-corruption bug documented in the
alpha.4 changelog.

**Re-targeted from alpha.5 → alpha.6 because the named prerequisite
("in-app helper text shipped first") did not land in alpha.5.** The
sanitize broadening in alpha.5 widened the set of sanitization
transformations (path-separator replacement, leading-dot stripping,
Windows-reserved-char replacement), making the dual-search fallback
load-bearing for *more* potential pre-alpha.5 names, not fewer. The
fallback removal must be paired with helper text that names the
specific transformations users might be affected by, otherwise
deleting the fallback is a silent UX regression for any user whose
wallet name produced different sanitize output before and after
alpha.5.

Deletion checklist (when the prerequisite ships):

1. Ship in-app helper text on the unlock screen pointing at "if you
   created a wallet on alpha.1–alpha.4 and don't see it listed, rename
   the file using the new sanitization rules"; include the broadened-
   policy examples (`My Wallet` → `My_Wallet`, `wallet:bak` →
   `wallet_bak`, `.hidden` → `hidden`).
2. Remove the raw-filename fallback in `src-tauri/src/commands.rs`
   (the `if sanitized_path_exists { … } else { raw_path }` block and
   the `raw_has_separator` guard).
3. Remove the helper text once one minor-release window has passed
   without user reports.
4. Update `CHANGELOG.md` under the release that removes it.

---

## Adopt `Engine::start_refresh` / `RefreshHandle` — target: post-wallet rewrite

Forward-tracking shekyl-core's pending wallet-engine migration.
`docs/design/STAGE_1_PR_4_REFRESH_ENGINE.md` (and its preflight notes)
define a pure-Rust refresh engine that replaces the C++
`wallet2::refresh` loop with an isolated process driving
`LedgerIndexes::process_scanned_outputs` and reorg handling. ~~The GUI
currently runs an in-process local sync loop in `wallet_bridge.rs`
(landed in alpha.5); when the engine lands in core and reaches feature
parity with the C++ `wallet2` FFI, the GUI's local loop is replaced
by a thin `RefreshHandle` client.~~ **Done** — the engine landed and
GUI-PR1 moved the GUI onto it: refresh is `Engine::start_refresh` driven
from `engine_session.rs`, the GUI-owned sync loop and `wallet_bridge.rs`
are deleted, and scan state lives inside the Engine rather than in a
GUI-held mutex.

**No GUI implementation work today.** `STAGE_1_PR_4` is design-only;
adopting it now would build against an unmerged API. Tracked so the
in-process loop's deletion target is named.

**Reversion criteria.** Adopt when **all** of:

1. `shekyl-core` ships a working `Engine` with feature parity to the
   GUI's current `Wallet2` FFI surface (specifically: `get_balance`,
   `get_address`, `query_key`, `transfer_native`, `stake`,
   `claim_rewards`, `get_transfers`).
2. The engine carries its own `start_refresh` / `RefreshHandle`
   surface (the GUI doesn't need to write a custom driver for the
   refresh loop).
3. The migration path is documented (config knob, side-by-side
   support window, deletion target for the C++ FFI shim).

---

## Adopt `PendingTxEngine` / `TxToSign` shape — target: post-`STAGE_1_PR_5`

Forward-tracking shekyl-core's pending transaction-signing migration.
`docs/design/STAGE_1_PR_5_PENDING_TX_ENGINE.md` defines a pure-Rust
pending-tx engine. The GUI's `transfer_native` path today is
"C++ wallet2 prepare → Rust sign → C++ finalize"; `STAGE_1_PR_5`
moves the bridge fully into Rust and reduces the surface to an
"engine returns `TxToSign`, GUI signs, engine finalizes" handshake.

**No GUI implementation work today.** The `STAGE_1_PR_5` design is
not yet merged; adopting it now would build against an unmerged API.

**Reversion criteria.** Adopt when `STAGE_1_PR_5` lands in
`shekyl-core/dev` and the new engine is paired with a working FFI
or Rust client surface the GUI can consume without the legacy
`transfer_native` path.

---

## Track `RANDOMX_V2_RUST.md` Phase 2+ — target: when phase 2 lands

Forward-tracking shekyl-core's RandomX v2 Rust verifier work.
`docs/design/RANDOMX_V2_RUST.md` Phase 1 (bundled `shekyld` build
wiring) landed in alpha.5 with no GUI behaviour change. Phase 2+ adds
the pure-Rust verifier; when it lands the bundled daemon switches
over and the GUI's `daemon_rpc.rs` may gain a verification-status
RPC field for the unlock/dashboard surface.

**No GUI implementation work today.** Phase 2 is build-wiring only
in core; consensus activation is gated on validation that the verifier
matches the C++ implementation across the RandomX test vector suite.

**Reversion criteria.** Re-evaluate when **any one of**:

1. `shekyl-core` lands the Rust verifier as the active path.
2. The daemon JSON-RPC contract gains a field exposing verifier
   identity / version that the GUI surfaces.
3. A consensus hard-fork activates the new verifier, requiring the
   bundled `shekyld` to migrate in lockstep.

---

## ~~Track `WALLET_REWRITE_PLAN.md` — target: post-V3.x~~ — **CLOSED**

This was the umbrella deletion target for the GUI's C++ `wallet2`
dependency: "when the rewrite reaches feature parity, the GUI drops
`shekyl-ffi` / `shekyl-engine-rpc`'s C++ surface and links the Rust engine
directly."

**Done, in two steps.** GUI-PR1 replaced `wallet_bridge.rs` with
`engine_session.rs` embedding `shekyl-engine-core::Engine` in-process,
dropping the `shekyl-ffi` / `shekyl-engine-rpc` deps and the C++ static
linkage. `shekyl-core` then deleted the `shekyl-engine-rpc` crate outright
(roadmap B1), so there is no longer a C++ wallet surface for this repo to
depend on even in principle. Nothing in the GUI process links C++ wallet
code.

**Residual, tracked elsewhere:** feature parity is not complete — the
capabilities that only ever existed on the old path (import-from-keys, PQC
multisig, scanner freeze/thaw) return honest "not available on the Engine
backend" errors and are carried by the per-PR followups above, not by this
entry. The *dependency* question this entry existed to answer is settled.

---

## Atomic amounts serialized as JS `number` — target: V3.2

Every balance the Tauri layer hands the frontend is a Rust `u64` of
atomic units serialized to a JS `number`: `Balance.{total,unlocked,
staked}` (`get_balance`), `DrainBalance.spendable`
(`get_drain_balance`, DS-PR-3 PR-B), and the `StakingView` legs +
`StakedOutputView.amount` (`get_staking_view`, GUI-PR3b — same
display-only disposition). JS `number` is IEEE-754 double —
exact only to 2^53 (≈ 9.007e15 atomic ≈ 9.0M SKL at 1e9 atomic/SKL).
Above that, the low-order atomic digits round in the JSON bridge.

**Why it's deferred, not fixed in DS-PR-3 PR-B.** The exposure is
*display-only* — these figures are rendered, never round-tripped into
transaction arithmetic (a drain/transfer computes its amounts in core
Rust `u64`; the GUI's `Send` path parses user input with `BigInt`
independently). And the SKL formatters (`formatSkl` 6-dp, `formatSklCompact`
K/M) are coarser than the `number` ULP across the entire supply range
(max supply 4.29e9 SKL → ULP at that magnitude ≈ the 6-dp display
granularity), so the rounding is not observable in any rendered value.
Patching one field to string+BigInt would need a divergent BigInt
formatter and leave `Balance` inconsistent beside it — tech-debt-shaped,
not tech-debt-removing (rules 15/16).

**The fix (systemic).** Migrate the balance-read pipeline wholesale:
serialize atomic amounts as decimal strings, type them `string` in
`daemon.ts`, parse with `BigInt`, and add a BigInt-native SKL formatter
that `Balance` and `DrainBalance` share. One PR, one consistent surface.

**Reversion criteria (bring forward from V3.2).** Any one of:

1. A drainable/balance figure begins seeding a transaction amount (e.g.
   a "drain max" button that prefills the send field from `spendable`) —
   at that point precision becomes arithmetic-load-bearing, not display.
2. A realistic single-wallet balance is expected to exceed ~9M SKL.
3. The daemon RPC contract migrates its own amount fields to strings and
   the GUI should follow in lockstep.

---

## npm / toolchain holds from the 2026-08-09 refresh — target: V3.1 / next Node bump

Taken in the `chore/npm-deps-refresh` window: lucide-react 1.x, TypeScript
6.0.3, ESLint 10, `@types/node` 26, jsdom 30, plugin-opener 2.5.4. Holds
with named reopen criteria:

1. **TypeScript 7.** `@latest` reports `7.0.2`, but `typescript-eslint@8`
   peers `typescript: '>=4.8.4 <6.1.0'`. Jumping to 7 breaks the lint
   pipeline. **Reopen when** typescript-eslint publishes a peer that
   admits TypeScript ≥7 (or a v9 that does), then bump TS + eslint
   together.
2. **React Compiler lint rules** (`react-hooks/set-state-in-effect`,
   `react-hooks/purity`). Folded into `recommended` in
   `eslint-plugin-react-hooks` ≥7.1; currently `off` in
   `eslint.config.js` because they still fire on ~10 intentional Tauri
   IPC poll / seed-challenge / `Date.now` sites after GUI-PR3b's
   YourStakePanel fix. **Reopen as** a dedicated cleanup PR that either
   rewrites those sites (e.g. `useSyncExternalStore` / event-driven
   refresh) or documents per-site exceptions — not as a silent
   re-enable on a dep bump.
