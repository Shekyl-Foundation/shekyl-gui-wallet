# Follow-ups

Items that don't fit the current release scope. Each has a target version.
Items without a target version get one within 30 days or get closed as
"won't fix." See `shekyl-core/docs/15-deletion-and-debt.mdc` for policy.

---

## Multisig integration — target: alpha.3

Pre-release audit (2026-04-15) found the multisig UI is scaffolding that
has not been re-verified against the reworked shekyl-core V3.1 multisig
API. The UI components shipped in alpha.1 as visual scaffolding; wiring
them to the backend is blocked on the items below.

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

## Pin shekyl-core by tag in release workflow — target: alpha.4

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

For alpha releases the gap is acceptable (the whole alpha pipeline is
still in motion), but before we tag anything users would hold long-term
(beta, stable, or any release labeled reproducible) the release workflow
must pin to a specific `shekyl-core` tag that matches the GUI wallet
version. Options, in order of preference:

1. **Matching-tag pin.** GUI wallet `v3.1.0-alpha.N` clones
   `shekyl-core` at tag `v3.1.0-alpha.N`. Requires that the shekyl-core
   tag exists before the GUI wallet tag is pushed. Makes the "which
   daemon ships in which wallet release" question trivially auditable.
2. **Pinned SHA.** The GUI wallet workflow reads a `SHEKYL_CORE_REV` file
   from its own repo and clones that exact commit. More flexible, less
   self-documenting, but decouples the two tag cadences.

CI (`ci.yml`) and CodeQL (`codeql.yml`) can stay on `dev` — they're
"does current dev still build against current dev" checks, which is
exactly what we want there.

Before closing this item: verify the release workflow actually passes
the pinned tag to all three platforms' checkout commands, and update
`docs/BUILD.md` (or equivalent) to document the pin policy for anyone
building from source.

---

## Remove `open_wallet` dual-search fallback — target: alpha.5

`commands::open_wallet` currently tries the sanitized filename first
(`"My_Wallet.keys"`) and falls back to the raw filename
(`"My Wallet.keys"`) if the sanitized variant is not present on disk.
This exists only to rescue wallets created against alpha.1–alpha.3
builds, which wrote filenames with spaces intact on Windows and
consequently produced the separator-corruption bug documented in the
Unreleased changelog.

Once users on those alphas have been prompted (via the in-app "Wallets
with spaces in their names have been renamed to use underscores"
helper text, not yet shipped) and at least one minor-release window
has passed, the raw-filename branch in `commands::open_wallet` should
be deleted. Rationale per `15-deletion-and-debt.mdc`: migration code
is permanent attack surface for a finite problem.

Deletion checklist:

1. Remove the raw-filename fallback in `src-tauri/src/commands.rs`
   (the `if sanitized_path_exists { … } else { raw_path }` block).
2. Remove the helper-text pointing at "wallets with spaces were
   renamed" from `Unlock.tsx` / release notes.
3. Update `CHANGELOG.md` under the release that removes it.

---

## Broaden `wallet_name::sanitize` character policy — target: alpha.5

`sanitize` currently only replaces whitespace with underscores. The
wider set of filesystem-unsafe characters on Windows (`<>:"/\?*`) is
rejected upstream by `validate::validate_wallet_name`, so they never
reach `sanitize`. That split works today but creates two places where
filename policy lives.

Before stable, collapse the two: `sanitize` should be the single
source of truth for "what does a filesystem-friendly Shekyl wallet
name look like" and `validate_wallet_name` should only check length
and non-empty after sanitization. This also lets us be more generous
with what the user can type (e.g. hyphens and dots) without risking
an error-message round trip.

Scope when this is picked up:

1. Decide the final allowed character class (recommend: Unicode
   letters/digits + `-_. ` with whitespace collapsed to `_`, all
   other characters replaced with `_` rather than rejected).
2. Update `sanitize` and its unit tests.
3. Simplify `validate_wallet_name` to post-sanitize-only checks.
4. Re-run proptest and fuzz corpora.

---

## Persist custom wallet directory across launches — target: alpha.5

`set_wallet_dir` / `reset_wallet_dir` currently only update in-memory
`AppState`; on next launch `init_wallet_rpc` re-derives the platform
default. For the Advanced picker to be useful long-term (e.g. an
encrypted-volume user who wants their wallets to live on
`D:\Vault\Shekyl`), the chosen directory needs to survive a restart.

Proposed shape:

1. Tauri app-config dir gets a tiny `gui-config.json`
   (`wallet_dir_override: Option<String>`). Not encrypted —
   it's a filepath, not a secret.
2. `init_wallet_rpc` reads the override before picking the default,
   and falls back to the default if the override is missing or the
   directory can't be accessed (e.g. external drive unplugged).
3. UI surfaces "override in effect, but directory not accessible"
   as a soft warning rather than a hard failure — the user can
   reset or choose a new location.

This is deliberately a separate item from the initial Advanced-picker
change so the persistence design can be reviewed on its own.
