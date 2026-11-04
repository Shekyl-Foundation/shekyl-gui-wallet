// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The wallet's one secret and its one spelling.
//!
//! A full wallet is [`PHRASE_WORD_COUNT`] BIP-39 English words, which are
//! the spelling of [`SEED_BYTES`] of entropy. Create draws the bytes and
//! spells them. Restore spells them back. [`crate::engine_session`] installs
//! the bytes and never sees the words.
//!
//! Shape refusals (empty, NUL, non-ASCII, a count other than
//! [`PHRASE_WORD_COUNT`], and the one-word-longer legacy phrase) name the
//! shape. A phrase of the right shape that does not decode gets one stable
//! message. No refusal repeats the input.

use rand::rngs::OsRng;
use rand::RngCore;
use shekyl_crypto_pq::account::SEED_BYTES;
use shekyl_crypto_pq::bip39::{entropy_from_mnemonic, mnemonic_from_entropy};
use zeroize::Zeroizing;

/// Words in the only recovery phrase Shekyl spells.
pub const PHRASE_WORD_COUNT: usize = 24;

/// A phrase one word longer than [`PHRASE_WORD_COUNT`]. Other wallets spell
/// that length; Shekyl does not.
const LEGACY_PHRASE_WORD_COUNT: usize = PHRASE_WORD_COUNT + 1;

const ERR_EMPTY: &str = "Recovery phrase must not be empty";
const ERR_NUL: &str = "Recovery phrase must not contain null bytes";
const ERR_NON_ASCII: &str = "Recovery phrase must use English letters only";
/// The digits in this sentence are [`PHRASE_WORD_COUNT`] and
/// [`LEGACY_PHRASE_WORD_COUNT`]. Kept as one literal so the spacing the
/// wallet already shows does not drift.
const ERR_LEGACY_PHRASE: &str = "Shekyl uses a 24-word recovery phrase. \
    25-word phrases from other wallets are not supported — Shekyl begins at its \
    own genesis and does not use legacy seed formats.";

/// The 32-byte wallet secret. Wrapped from the moment it exists.
pub type WalletSeed = Zeroizing<[u8; SEED_BYTES]>;

/// The words handed out once at creation. Private field: the only way out
/// is [`SeedBackup::into_mnemonic`], which moves the `String` and leaves an
/// empty one for the wrapper to wipe.
pub struct SeedBackup(Zeroizing<String>);

impl SeedBackup {
    /// Move the words onto the create response. No second copy.
    pub fn into_mnemonic(self) -> String {
        let SeedBackup(mut words) = self;
        std::mem::take(&mut *words)
    }
}

/// A fresh seed from the OS RNG and the words that spell it. Both are
/// wrapped before this returns. The words are never a bare `String`.
pub fn generate() -> Result<(WalletSeed, SeedBackup), String> {
    let mut seed = Zeroizing::new([0u8; SEED_BYTES]);
    OsRng.fill_bytes(seed.as_mut());
    let words = Zeroizing::new(
        mnemonic_from_entropy(&seed).map_err(|e| format!("mnemonic_from_entropy: {e}"))?,
    );
    Ok((seed, SeedBackup(words)))
}

/// Decode a recovery phrase to its seed.
///
/// The shape checks run first so each known bad shape keeps its message.
/// The decoder then accepts a real [`PHRASE_WORD_COUNT`]-word phrase and
/// refuses every other 24-token string with [`invalid_mnemonic_message`],
/// which does not include the decoder's own error.
pub fn seed_from_phrase(phrase: &str) -> Result<WalletSeed, String> {
    if phrase.is_empty() {
        return Err(ERR_EMPTY.into());
    }
    if phrase.contains('\0') {
        return Err(ERR_NUL.into());
    }
    if !phrase.is_ascii() {
        return Err(ERR_NON_ASCII.into());
    }

    let word_count = phrase.split_whitespace().count();
    if word_count == LEGACY_PHRASE_WORD_COUNT {
        return Err(ERR_LEGACY_PHRASE.into());
    }
    if word_count != PHRASE_WORD_COUNT {
        return Err(format!(
            "Recovery phrase must be exactly {PHRASE_WORD_COUNT} words"
        ));
    }

    entropy_from_mnemonic(phrase).map_err(|_| invalid_mnemonic_message())
}

fn invalid_mnemonic_message() -> String {
    format!("invalid mnemonic (expected {PHRASE_WORD_COUNT} BIP-39 words)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_words_decode_to_the_same_seed() {
        let (seed, backup) = generate().unwrap();
        let words = backup.into_mnemonic();
        assert_eq!(words.split_whitespace().count(), PHRASE_WORD_COUNT);
        let restored = seed_from_phrase(&words).unwrap();
        assert_eq!(*restored, *seed);
    }

    #[test]
    fn a_padded_phrase_decodes_to_the_same_seed() {
        let (seed, backup) = generate().unwrap();
        let words = backup.into_mnemonic();
        let restored = seed_from_phrase(&format!("  {words}  ")).unwrap();
        assert_eq!(*restored, *seed);
    }

    #[test]
    fn the_wrong_shape_is_named_and_the_input_is_not() {
        let (_seed, backup) = generate().unwrap();
        let words = backup.into_mnemonic();
        let twelve = words
            .split_whitespace()
            .take(PHRASE_WORD_COUNT / 2)
            .collect::<Vec<_>>()
            .join(" ");
        let hex = "ab".repeat(SEED_BYTES);

        let err = seed_from_phrase("").unwrap_err();
        assert_eq!(err, ERR_EMPTY);

        let err = seed_from_phrase("   ").unwrap_err();
        assert_eq!(
            err,
            format!("Recovery phrase must be exactly {PHRASE_WORD_COUNT} words")
        );

        let err = seed_from_phrase("café latté").unwrap_err();
        assert_eq!(err, ERR_NON_ASCII);
        assert!(!err.contains("caf"));

        let err = seed_from_phrase(&hex).unwrap_err();
        assert_eq!(
            err,
            format!("Recovery phrase must be exactly {PHRASE_WORD_COUNT} words")
        );
        assert!(!err.contains("ab"), "{err}");

        // Each refusal is a fixed message, so it cannot echo the input. A
        // substring check against a random word is not that test: a short
        // BIP-39 word ("use") can sit inside the fixed text ("uses").
        let err = seed_from_phrase(&twelve).unwrap_err();
        assert_eq!(
            err,
            format!("Recovery phrase must be exactly {PHRASE_WORD_COUNT} words")
        );

        let legacy = format!("{words} extra");
        let err = seed_from_phrase(&legacy).unwrap_err();
        assert_eq!(err, ERR_LEGACY_PHRASE);
    }

    #[test]
    fn twenty_four_tokens_that_do_not_decode_use_the_one_stable_message() {
        let (_, backup) = generate().unwrap();
        let words = backup.into_mnemonic();
        let mut parts: Vec<String> = words.split_whitespace().map(str::to_owned).collect();
        // A swapped last word still passes BIP-39's 8-bit checksum about one
        // time in 256, so take the first candidate that does not decode.
        // The refusal is one fixed message, so it cannot echo the input.
        let original = parts[PHRASE_WORD_COUNT - 1].clone();
        let err = ["abandon", "ability", "able", "about", "above", "absent"]
            .into_iter()
            .filter(|w| *w != original)
            .find_map(|w| {
                parts[PHRASE_WORD_COUNT - 1] = w.to_owned();
                seed_from_phrase(&parts.join(" ")).err()
            })
            .expect("one of six swaps fails the checksum");
        assert_eq!(err, invalid_mnemonic_message());
    }

    #[test]
    fn a_phrase_error_does_not_leak_the_words() {
        let canary = "abandon ability able about above absent absorb abstract absurd abuse \
                      access accident account accuse achieve acid acoustic acquire across act action";
        let err = seed_from_phrase(&format!("{canary}\0injected")).unwrap_err();
        assert_eq!(err, ERR_NUL);
        for leaked in ["abandon", "acoustic", "injected"] {
            assert!(!err.contains(leaked), "{err}");
        }
    }

    mod prop {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn seed_from_phrase_never_panics(s in "\\PC{0,1000}") {
                let _ = seed_from_phrase(&s);
            }

            #[test]
            fn a_long_phrase_cannot_fit_inside_its_refusal(s in "\\PC{300,400}") {
                let err = seed_from_phrase(&s).unwrap_err();
                prop_assert!(err.len() < s.len());
                prop_assert!(!err.contains(s.as_str()));
            }
        }
    }
}
