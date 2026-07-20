# Follow-ups

Items that don't fit the current release scope. Each has a target version.
Items without a target version get one within 30 days or get closed as
"won't fix." See `shekyl-core/docs/15-deletion-and-debt.mdc` for policy.

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
| GUI-PR4 | `stake_in` funding UX | public/RPC `stake_in` (core PR-P3+) | next |
| GUI-PR5 | Multisig address-fingerprint cutover | group_id deleted in core | pending |
| GUI-PR6+ | unbond / drain / live shards | PR-P4/P5/P6 + emission | pending |

**GUI-PR3 leftovers:** activation without stake_in funding often returns
not-ready (expected until PR4). No UI for multi-slot W2 resume detail.
Next: GUI-PR4 `stake_in` funding.

**Deleted:** the Wallet2 backend, `wallet_bridge`, the `shekyl-ffi` /
`shekyl-engine-rpc` deps and the C++ static linkage, the
`SHEKYL_ENGINE_BACKEND` flag, and claim-era `stake` / `claim_rewards` /
`validate_tier`. **Still to delete when done:** `StakeTierCard` if unused,
`get_tier_yields` if daemon tiers vanish, and the scanner-command honest-error
stubs once Engine-native archival queries exist.

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

- The `multisig` feature on `shekyl-engine-rpc` is not enabled in
  `src-tauri/Cargo.toml`, so the Rust FROST multisig handlers are not
  compiled into the wallet binary.
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
`LedgerIndexes::process_scanned_outputs` and reorg handling. The GUI
currently runs an in-process local sync loop in `wallet_bridge.rs`
(landed in alpha.5); when the engine lands in core and reaches feature
parity with the C++ `wallet2` FFI, the GUI's local loop is replaced
by a thin `RefreshHandle` client.

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

## Track `WALLET_REWRITE_PLAN.md` — target: post-V3.x

Placeholder for the eventual deletion of `shekyl-engine-rpc`'s
`wallet2` FFI dependency in favor of the pure-Rust `Engine` (the
combination of `STAGE_1_PR_3` Key Engine, `STAGE_1_PR_4` Refresh
Engine, `STAGE_1_PR_5` Pending Tx Engine, and follow-ups). When the
rewrite reaches feature parity, the GUI drops `shekyl-ffi` /
`shekyl-engine-rpc`'s C++ surface and links the Rust engine directly.

**No GUI implementation work today.** The wallet rewrite is staged
across multiple `shekyl-core` PRs and a multi-quarter timeline.

**Reversion criteria.** Track via the per-PR followups above; this
entry is the umbrella deletion target for the C++ `wallet2`
dependency once all per-PR migrations are complete.
