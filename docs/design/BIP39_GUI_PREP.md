# BIP-39 GUI prep — shekyl-gui-wallet

**Status:** Prep landed; **Engine path integration landed in GUI-PR1**.

Create/restore on the pure-Rust Engine backend uses BIP-39 via
`shekyl-crypto-pq` (`generate_account_from_bip39` / `mnemonic_from_entropy`)
— **not** `wallet2_ffi_*` symbols. The Engine is the sole wallet backend; the
transitional Wallet2 path and the `SHEKYL_ENGINE_BACKEND` flag have been
removed.

**Authority:**

- `shekyl-core/docs/design/ELECTRUM_WORDS_REMOVAL.md` §3.2.1
- `shekyl-core` `shekyl-wallet-rpc` lifecycle (same BIP-39 create path)
- GUI plan: Rust-forward / FFI collapse

## What this prep PR delivers

| Area | Change |
| --- | --- |
| `src-tauri/src/validate.rs` | `validate_recovery_phrase` (24 words; 25-word legacy rejection) |
| `src-tauri/src/commands.rs` | `import_wallet_from_seed` calls `validate_recovery_phrase` before FFI |
| `src/pages/ImportWallet.tsx` | 24-word UI, info banner, optional passphrase field (state only) |
| `src/pages/*` | Copy: recovery phrase, 24 words |
| `src/constants/wallet.ts` | `BIP39_RECOVERY_PHRASE_WORD_COUNT = 24` |
| Docs | `USER_GUIDE.md`, `WALLET_STARTUP.md`, this file |

## Explicitly not in prep

- No `wallet_bridge.rs` / wallet2 FFI changes
- No `shekyl_account_*` or `shekyl-engine-file` usage
- No removal of `language` / `seed_language` from Rust Tauri types
- No functional mainnet create or BIP-39 restore

## Integration PR checklist (after core)

### Core readiness (verify before integration PR)

- [ ] `wallet2_ffi_create_wallet_from_bip39` exists; `wallet2_ffi_create_wallet` deleted
- [ ] BIP-39 restore FFI exists (provisional name `wallet2_ffi_restore_from_bip39`);
      `wallet2_ffi_restore_deterministic_wallet` deleted
- [ ] `shekyl-engine-rpc` exposes matching `Wallet2` methods
- [ ] Mainnet: create → `query_key("mnemonic")` returns 24 words; restore round-trip same address
- [ ] `shekyl-core` `CHANGELOG.md` updated

### GUI integration tasks

1. **`wallet_bridge.rs`** — `create_wallet_from_bip39`, `restore_from_bip39`; delete Electrum wrappers
2. **`commands.rs`** — drop `language` / `seed_language`; pass `passphrase` to restore FFI
3. **`WalletContext.tsx`** — remove `language` from invoke payloads
4. **UI** — remove prep banners; wire passphrase to restore command
5. **E2E manual** — mainnet create → backup → close → restore → same address

## API contract (draft)

| GUI need | Core FFI (provisional) | Parameters |
| --- | --- | --- |
| Create | `wallet2_ffi_create_wallet_from_bip39` | `wallet_path`, `password` |
| Restore | `wallet2_ffi_restore_from_bip39` | `wallet_path`, `mnemonic`, `password`, `passphrase`, `restore_height` |
| Backup display | `wallet2_ffi_query_key` | `key_type = "mnemonic"`, empty passphrase |

## Reversion / release posture

If core FFI slips past a named milestone (e.g. stressnet wallet build):

- Prep UX (24-word copy, validation) remains valid
- Keep info banners until integration ships
- Do not wire old `wallet2_ffi_create_wallet` / Electrum restore as a fallback
