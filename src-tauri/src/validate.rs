// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Input validation for Tauri bridge commands.
//!
//! Every command that accepts user input must validate before hitting
//! the C++ wallet2 FFI. A malformed destination address or amount that
//! reaches C++ is a DoS at best, memory corruption at worst.

use shekyl_address::ShekylAddress;

const MAX_WALLET_NAME_LEN: usize = 255;
const MAX_PASSWORD_LEN: usize = 1024;
const MAX_MNEMONIC_WORDS: usize = 30;

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

/// Validate a wallet filename.
pub fn validate_wallet_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Wallet name must not be empty".into());
    }
    if name.len() > MAX_WALLET_NAME_LEN {
        return Err(format!(
            "Wallet name too long (max {MAX_WALLET_NAME_LEN} chars)"
        ));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("Wallet name must not contain path separators or null bytes".into());
    }
    if name.starts_with('.') {
        return Err("Wallet name must not start with a dot".into());
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

/// Validate a mnemonic seed phrase.
pub fn validate_seed(seed: &str) -> Result<(), String> {
    if seed.is_empty() {
        return Err("Seed phrase must not be empty".into());
    }
    let word_count = seed.split_whitespace().count();
    if word_count == 0 || word_count > MAX_MNEMONIC_WORDS {
        return Err(format!(
            "Seed must have 1-{MAX_MNEMONIC_WORDS} words, got {word_count}"
        ));
    }
    if !seed.is_ascii() {
        return Err("Seed phrase must be ASCII".into());
    }
    if seed.contains('\0') {
        return Err("Seed phrase must not contain null bytes".into());
    }
    Ok(())
}

/// Validate a key image hex string (32 bytes = 64 hex chars).
pub fn validate_key_image(key_image: &str) -> Result<(), String> {
    validate_hex(key_image, 32, "key_image")
}

/// Validate a secret key hex string (32 bytes = 64 hex chars).
pub fn validate_secret_key(key: &str, name: &str) -> Result<(), String> {
    validate_hex(key, 32, name)
}

/// Validate a staking tier.
pub fn validate_tier(tier: u8) -> Result<(), String> {
    if tier > 2 {
        return Err(format!("Invalid staking tier: {tier}. Must be 0, 1, or 2"));
    }
    Ok(())
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
    fn reject_wallet_name_with_path_separator() {
        assert!(validate_wallet_name("../evil").is_err());
        assert!(validate_wallet_name("evil\\name").is_err());
    }

    #[test]
    fn reject_wallet_name_starting_with_dot() {
        assert!(validate_wallet_name(".hidden").is_err());
    }

    #[test]
    fn accept_valid_wallet_name() {
        assert!(validate_wallet_name("my_wallet").is_ok());
    }

    #[test]
    fn reject_null_in_password() {
        assert!(validate_password("pass\0word").is_err());
    }

    #[test]
    fn reject_empty_seed() {
        assert!(validate_seed("").is_err());
    }

    #[test]
    fn reject_non_ascii_seed() {
        assert!(validate_seed("café latté").is_err());
    }

    #[test]
    fn accept_valid_seed() {
        assert!(validate_seed("word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 word14 word15 word16 word17 word18 word19 word20 word21 word22 word23 word24 word25").is_ok());
    }

    #[test]
    fn reject_invalid_tier() {
        assert!(validate_tier(3).is_err());
        assert!(validate_tier(255).is_err());
    }

    #[test]
    fn accept_valid_tier() {
        assert!(validate_tier(0).is_ok());
        assert!(validate_tier(1).is_ok());
        assert!(validate_tier(2).is_ok());
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
    fn wallet_name_error_does_not_leak_path_secret() {
        let evil_name = format!("../../../{CANARY_SHORT}/secrets");
        let err = validate_wallet_name(&evil_name).unwrap_err();
        assert_no_canary(&err, &[CANARY_SHORT]);
    }

    #[test]
    fn password_null_error_does_not_leak_content() {
        let evil_pass = format!("{CANARY_SHORT}\0rest_of_password");
        let err = validate_password(&evil_pass).unwrap_err();
        assert_no_canary(&err, &[CANARY_SHORT, "rest_of_password"]);
    }

    #[test]
    fn seed_error_does_not_leak_words() {
        let canary_seed = "abandon ability able about above absent absorb abstract absurd abuse \
                           access accident account accuse achieve acid acoustic acquire across act action";
        let err = validate_seed(&format!("{canary_seed}\0injected")).unwrap_err();
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
            fn validate_seed_never_panics(s in "\\PC{0,1000}") {
                let _ = validate_seed(&s);
            }

            #[test]
            fn validate_key_image_never_panics(s in "\\PC{0,200}") {
                let _ = validate_key_image(&s);
            }

            #[test]
            fn validate_tier_never_panics(t: u8) {
                let _ = validate_tier(t);
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
