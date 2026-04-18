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

- The `multisig` feature on `shekyl-wallet-rpc` is not enabled in
  `src-tauri/Cargo.toml`, so the Rust FROST multisig handlers are not
  compiled into the wallet binary.
- `export_group_descriptor` produces mostly-empty data because
  `participant_pubkeys` and `address_fingerprint` come from JSON fields
  the backend doesn't return.
- The V3.1 dashboard (intents, violations, relay status, prover view) is
  pure scaffolding — state is initialized but never populated from backend
  commands that don't exist yet.

### Type mismatches

- `SigningDashboard` expects lowercase `IntentState` (`"proposed"`,
  `"signed"`, etc.); the page's state types previously used PascalCase.
  Fixed in alpha.2 to lowercase to match the component, but the backend
  wire format needs to be confirmed when the signing flow is implemented.
- `ProverView` expects `amount: string`; page state previously used
  `number`. Fixed in alpha.2.
- `ViolationAlert` expects `invariantId: number`; page state previously
  used `string`. Fixed in alpha.2.

### Plan

Create a dedicated multisig integration plan for alpha.3 that:

1. Implements `sign_multisig_partial` in `wallet2_ffi.cpp`.
2. Extends `get_pqc_multisig_info` to return all fields the UI needs.
3. Enables the `multisig` feature in the GUI wallet's Cargo.toml.
4. Wires the Rust FROST handlers to the Tauri command layer.
5. Connects the UI components to real backend data.
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
