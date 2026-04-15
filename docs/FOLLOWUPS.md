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
