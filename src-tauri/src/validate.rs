// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Input validation for Tauri bridge commands.
//!
//! Every command that accepts user input must validate before it reaches
//! the Engine backend. A malformed destination address or amount that slips
//! through is a correctness hazard at best and a denial-of-service at worst.

use shekyl_address::ShekylAddress;

const MAX_WALLET_NAME_LEN: usize = 255;
const MAX_PASSWORD_LEN: usize = 1024;

/// BIP-39 English recovery phrase length for Shekyl genesis wallets.
pub const BIP39_RECOVERY_PHRASE_WORD_COUNT: usize = 24;

/// User-facing rejection when a legacy 25-word phrase is supplied.
/// Aligned with shekyl-core V6 hint (`wallet2.cpp` parse_wallet_create_data).
const ERR_LEGACY_25_WORD_PHRASE: &str = "Shekyl uses a 24-word recovery phrase. \
    25-word phrases from other wallets are not supported — Shekyl begins at its \
    own genesis and does not use legacy seed formats.";

/// Validate a Shekyl address string.
///
/// Parses the Bech32m-encoded address and returns Ok(()) if valid.
/// Rejects empty, malformed, or non-Shekyl addresses.
pub fn validate_address(address: &str) -> Result<(), String> {
    if address.is_empty() {
        return Err("Address must not be empty".into());
    }
    if address.len() > 4096 {
        return Err("Address is too long".into());
    }
    ShekylAddress::decode(address).map_err(|e| format!("Invalid address: {e}"))?;
    Ok(())
}

/// Validate a transfer amount (in atomic units).
pub fn validate_amount(amount: u64) -> Result<(), String> {
    if amount == 0 {
        return Err("Amount must be greater than zero".into());
    }
    Ok(())
}

/// Validate a hex string of expected byte length.
pub fn validate_hex(hex_str: &str, expected_bytes: usize, field_name: &str) -> Result<(), String> {
    if hex_str.len() != expected_bytes * 2 {
        return Err(format!(
            "{field_name} must be {} hex chars, got {}",
            expected_bytes * 2,
            hex_str.len()
        ));
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{field_name} contains non-hex characters"));
    }
    Ok(())
}

/// Validate a wallet filename **after** it has been normalized by
/// [`crate::wallet_name::sanitize`].
///
/// Filename policy (path-separator rejection, leading-dot stripping,
/// Windows-reserved-char replacement, null-byte rejection, Unicode-
/// letter preservation) is owned entirely by `sanitize` —
/// see its rustdoc for the full set. By the time a name reaches this
/// function it is already filesystem-safe; the only failure modes left
/// are "input collapsed to empty" and "input exceeds
/// [`MAX_WALLET_NAME_LEN`]".
///
/// Callers must wrap raw user input as:
///
/// ```ignore
/// let sanitized = wallet_name::sanitize(&raw);
/// validate::validate_wallet_name(&sanitized)?;
/// ```
///
/// Passing raw (unsanitized) input here means accepting whatever the
/// user typed and letting `sanitize`'s policy do the work; this
/// function is intentionally permissive about ASCII shape so that a
/// previously-stored wallet whose name uses an idiosyncratic character
/// set still loads.
pub fn validate_wallet_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Wallet name must not be empty".into());
    }
    if name.len() > MAX_WALLET_NAME_LEN {
        return Err(format!(
            "Wallet name too long (max {MAX_WALLET_NAME_LEN} chars)"
        ));
    }
    Ok(())
}

/// Validate a wallet password.
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() > MAX_PASSWORD_LEN {
        return Err(format!("Password too long (max {MAX_PASSWORD_LEN} chars)"));
    }
    if password.contains('\0') {
        return Err("Password must not contain null bytes".into());
    }
    Ok(())
}

/// Validate a BIP-39 English recovery phrase (24 words) for import/restore.
///
/// Error messages describe the failure class only; the phrase is never echoed.
pub fn validate_recovery_phrase(phrase: &str) -> Result<(), String> {
    if phrase.is_empty() {
        return Err("Recovery phrase must not be empty".into());
    }
    if phrase.contains('\0') {
        return Err("Recovery phrase must not contain null bytes".into());
    }
    if !phrase.is_ascii() {
        return Err("Recovery phrase must use English letters only".into());
    }

    let word_count = phrase.split_whitespace().count();
    if word_count == BIP39_RECOVERY_PHRASE_WORD_COUNT {
        return Ok(());
    }
    if word_count == 25 {
        return Err(ERR_LEGACY_25_WORD_PHRASE.into());
    }
    Err(format!(
        "Recovery phrase must be exactly {BIP39_RECOVERY_PHRASE_WORD_COUNT} words"
    ))
}

/// Validate a key image hex string (32 bytes = 64 hex chars).
pub fn validate_key_image(key_image: &str) -> Result<(), String> {
    validate_hex(key_image, 32, "key_image")
}

/// Validate a secret key hex string (32 bytes = 64 hex chars).
pub fn validate_secret_key(key: &str, name: &str) -> Result<(), String> {
    validate_hex(key, 32, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_empty_address() {
        assert!(validate_address("").is_err());
    }

    #[test]
    fn reject_oversized_address() {
        let long = "a".repeat(5000);
        assert!(validate_address(&long).is_err());
    }

    #[test]
    fn reject_zero_amount() {
        assert!(validate_amount(0).is_err());
    }

    #[test]
    fn accept_nonzero_amount() {
        assert!(validate_amount(1).is_ok());
        assert!(validate_amount(u64::MAX).is_ok());
    }

    #[test]
    fn validate_hex_correct_length() {
        let hex64 = "a".repeat(64);
        assert!(validate_hex(&hex64, 32, "test").is_ok());
    }

    #[test]
    fn reject_hex_wrong_length() {
        assert!(validate_hex("abcd", 32, "test").is_err());
    }

    #[test]
    fn reject_hex_non_hex_chars() {
        let bad = "zz".to_string() + &"0".repeat(62);
        assert!(validate_hex(&bad, 32, "test").is_err());
    }

    #[test]
    fn reject_empty_wallet_name() {
        // Post-sanitize empty (e.g. all-forbidden input like "////")
        // surfaces here as the empty-string check.
        assert!(validate_wallet_name("").is_err());
    }

    #[test]
    fn reject_oversize_wallet_name() {
        let long = "a".repeat(MAX_WALLET_NAME_LEN + 1);
        assert!(validate_wallet_name(&long).is_err());
    }

    #[test]
    fn accept_valid_wallet_name() {
        assert!(validate_wallet_name("my_wallet").is_ok());
        assert!(validate_wallet_name("MyWallet").is_ok());
        assert!(validate_wallet_name("café_wallet").is_ok());
    }

    // Path-separator / leading-dot / null-byte rejection now lives in
    // `wallet_name::sanitize` (single source of truth). The tests for
    // those behaviours have moved to `src/wallet_name.rs`:
    //
    //   - sanitize_replaces_path_separators
    //   - sanitize_strips_leading_dots
    //   - sanitize_replaces_null_bytes
    //   - sanitize_strips_windows_reserved_chars
    //   - sanitize_output_contains_only_allowed_chars (proptest)
    //
    // Production code paths in `commands.rs` already pipe raw user
    // input through `sanitize` *before* calling `validate_wallet_name`,
    // so the policy is enforced end-to-end.

    #[test]
    fn reject_null_in_password() {
        assert!(validate_password("pass\0word").is_err());
    }

    fn twenty_four_word_phrase() -> String {
        (1..=BIP39_RECOVERY_PHRASE_WORD_COUNT)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn reject_empty_recovery_phrase() {
        assert!(validate_recovery_phrase("").is_err());
    }

    #[test]
    fn reject_non_ascii_recovery_phrase() {
        assert!(validate_recovery_phrase("café latté").is_err());
    }

    #[test]
    fn accept_valid_recovery_phrase() {
        assert!(validate_recovery_phrase(&twenty_four_word_phrase()).is_ok());
    }

    #[test]
    fn reject_25_word_recovery_phrase() {
        let mut phrase = twenty_four_word_phrase();
        phrase.push_str(" extra");
        let err = validate_recovery_phrase(&phrase).unwrap_err();
        assert_eq!(err, ERR_LEGACY_25_WORD_PHRASE);
    }

    #[test]
    fn reject_wrong_word_count_recovery_phrase() {
        let short = (1..23)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let err = validate_recovery_phrase(&short).unwrap_err();
        assert!(err.contains("exactly 24"), "unexpected error: {err}");
    }

    // ── Gate 6: Canary-based secret leak detection ──
    //
    // These tests plant known canary byte patterns in input fields that could
    // plausibly be secrets, feed them to validation functions that reject them,
    // and assert the returned error strings contain NONE of the canaries.
    // This catches secrets leaking through debug-print, format strings, or
    // error message interpolation.

    const CANARY_HEX: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    const CANARY_SHORT: &str = "deadbeef";

    fn assert_no_canary(err: &str, canaries: &[&str]) {
        for canary in canaries {
            assert!(
                !err.to_lowercase().contains(&canary.to_lowercase()),
                "error message leaked canary '{}' in: {}",
                canary,
                err
            );
        }
    }

    #[test]
    fn address_error_does_not_leak_input() {
        let bad_addr = format!("shekyl1{CANARY_HEX}");
        let err = validate_address(&bad_addr).unwrap_err();
        assert_no_canary(&err, &[CANARY_HEX, CANARY_SHORT]);
    }

    #[test]
    fn hex_error_does_not_leak_input() {
        let err = validate_hex(CANARY_HEX, 16, "test_field").unwrap_err();
        assert_no_canary(&err, &[CANARY_HEX, CANARY_SHORT]);
    }

    #[test]
    fn key_image_error_does_not_leak_canary() {
        let short_ki = &CANARY_HEX[..32];
        let err = validate_key_image(short_ki).unwrap_err();
        assert_no_canary(&err, &[short_ki, CANARY_SHORT]);
    }

    #[test]
    fn secret_key_error_does_not_leak_canary() {
        let short_sk = &CANARY_HEX[..32];
        let err = validate_secret_key(short_sk, "spend_key").unwrap_err();
        assert_no_canary(&err, &[short_sk, CANARY_SHORT]);
    }

    #[test]
    fn wallet_name_error_does_not_leak_canary() {
        // `validate_wallet_name` only rejects empty / oversize after
        // `wallet_name::sanitize` has scrubbed path traversal,
        // separators, null bytes, etc. (see this module's rustdoc).
        // The leak-canary guarantee remains: the oversize error
        // message does not echo any of the input back to the caller.
        let evil = format!("{}{CANARY_HEX}", "X".repeat(MAX_WALLET_NAME_LEN));
        let err = validate_wallet_name(&evil).unwrap_err();
        assert_no_canary(&err, &[CANARY_HEX, CANARY_SHORT, "XXXXX"]);
    }

    #[test]
    fn password_null_error_does_not_leak_content() {
        let evil_pass = format!("{CANARY_SHORT}\0rest_of_password");
        let err = validate_password(&evil_pass).unwrap_err();
        assert_no_canary(&err, &[CANARY_SHORT, "rest_of_password"]);
    }

    #[test]
    fn recovery_phrase_error_does_not_leak_words() {
        let canary_seed = "abandon ability able about above absent absorb abstract absurd abuse \
                           access accident account accuse achieve acid acoustic acquire across act action";
        let err = validate_recovery_phrase(&format!("{canary_seed}\0injected")).unwrap_err();
        assert_no_canary(&err, &["abandon", "acoustic", "injected"]);
    }

    #[test]
    fn oversized_address_error_does_not_leak_content() {
        let big = "X".repeat(5000);
        let err = validate_address(&big).unwrap_err();
        assert!(
            !err.contains("XXXXX"),
            "error leaked oversized input content"
        );
    }

    #[test]
    fn adversarial_format_string_in_address() {
        let evil = "shekyl1%s%s%s%n%n%n{:?}{{}}";
        let err = validate_address(evil).unwrap_err();
        assert!(!err.contains("%n"), "format string injection not sanitized");
        assert!(!err.contains("{:?}"), "Rust debug format specifier leaked");
    }

    // ── Proptest: fuzz validators never panic or leak input ──

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn validate_address_never_panics(s in "\\PC{0,5000}") {
                let _ = validate_address(&s);
            }

            #[test]
            fn validate_amount_never_panics(a: u64) {
                let _ = validate_amount(a);
            }

            #[test]
            fn validate_hex_never_panics(s in "\\PC{0,200}", len in 0usize..100) {
                let _ = validate_hex(&s, len, "fuzz");
            }

            #[test]
            fn validate_wallet_name_never_panics(s in "\\PC{0,500}") {
                let _ = validate_wallet_name(&s);
            }

            #[test]
            fn validate_password_never_panics(s in "\\PC{0,2000}") {
                let _ = validate_password(&s);
            }

            #[test]
            fn validate_recovery_phrase_never_panics(s in "\\PC{0,1000}") {
                let _ = validate_recovery_phrase(&s);
            }

            #[test]
            fn validate_key_image_never_panics(s in "\\PC{0,200}") {
                let _ = validate_key_image(&s);
            }

            #[test]
            fn error_messages_never_contain_full_input(s in "[a-f0-9]{64,128}") {
                if let Err(e) = validate_address(&s) {
                    assert!(
                        !e.contains(&s),
                        "address error leaked full input"
                    );
                }
                if let Err(e) = validate_wallet_name(&s) {
                    assert!(
                        !e.contains(&s),
                        "wallet_name error leaked full input"
                    );
                }
            }
        }
    }
}
