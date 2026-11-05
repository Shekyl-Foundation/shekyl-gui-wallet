// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! The wallet's one secret and its one spelling.
//!
//! A full wallet is [`SHEKYL_MNEMONIC_WORD_COUNT`] BIP-39 English words,
//! the spelling of [`SEED_BYTES`] of entropy. Create draws the bytes and
//! spells them. Restore asks [`entropy_from_mnemonic`] and turns its
//! [`PhraseError`] into a sentence. [`crate::engine_session`] installs the
//! bytes and never sees the words.
//!
//! A null byte is refused here, before the decoder, the same screen a
//! password gets. Every other refusal is the decoder's class. No refusal
//! repeats the input.

use rand::rngs::OsRng;
use rand::RngCore;
use shekyl_crypto_pq::account::SEED_BYTES;
use shekyl_crypto_pq::bip39::{
    entropy_from_mnemonic, mnemonic_from_entropy, PhraseError, SHEKYL_MNEMONIC_WORD_COUNT,
};
use zeroize::Zeroizing;

/// A phrase one word longer than [`SHEKYL_MNEMONIC_WORD_COUNT`]. Other
/// wallets spell that length; Shekyl does not.
const LEGACY_WORD_COUNT: usize = SHEKYL_MNEMONIC_WORD_COUNT + 1;

const ERR_EMPTY: &str = "Recovery phrase must not be empty";
const ERR_NUL: &str = "Recovery phrase must not contain null bytes";
const ERR_UNKNOWN_WORD: &str =
    "One of these words is not a recovery-phrase word. Check the words you entered.";
const ERR_CHECKSUM: &str =
    "These words do not match a recovery phrase. Check the words and their order.";
const ERR_UNREADABLE: &str = "The recovery phrase could not be read. Try again.";
const ERR_CREATE: &str = "The wallet could not be created. Try again.";

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
///
/// A failure here is the speller, not the user. The page sees
/// [`ERR_CREATE`]; the speller's own error is logged and not returned.
pub fn generate() -> Result<(WalletSeed, SeedBackup), String> {
    let mut seed = Zeroizing::new([0u8; SEED_BYTES]);
    OsRng.fill_bytes(seed.as_mut());
    let spelled = mnemonic_from_entropy(&seed).map_err(|e| {
        tracing::warn!(error = %e, "spelling a new seed failed");
        ERR_CREATE.to_owned()
    })?;
    Ok((seed, SeedBackup(Zeroizing::new(spelled))))
}

/// Decode a recovery phrase to its seed.
///
/// A null byte is refused before the decoder. Every other input is
/// [`entropy_from_mnemonic`]'s answer, mapped by [`refusal_message`].
pub fn seed_from_phrase(phrase: &str) -> Result<WalletSeed, String> {
    if phrase.contains('\0') {
        return Err(ERR_NUL.into());
    }
    entropy_from_mnemonic(phrase).map_err(refusal_message)
}

/// The sentence for one [`PhraseError`]. The count arms name the length
/// Shekyl spells. A 24-word phrase that fails the wordlist or the checksum
/// names that failure, not the length the caller already got right.
fn refusal_message(err: PhraseError) -> String {
    match err {
        PhraseError::WordCount(0) => ERR_EMPTY.to_owned(),
        PhraseError::WordCount(LEGACY_WORD_COUNT) => legacy_phrase_message(),
        PhraseError::WordCount(_) | PhraseError::Length(_) => word_count_message(),
        PhraseError::UnknownWord {
            words: SHEKYL_MNEMONIC_WORD_COUNT,
        } => ERR_UNKNOWN_WORD.to_owned(),
        PhraseError::Checksum {
            words: SHEKYL_MNEMONIC_WORD_COUNT,
        } => ERR_CHECKSUM.to_owned(),
        PhraseError::UnknownWord { .. } | PhraseError::Checksum { .. } => word_count_message(),
        // `EntropyLength` and `Unclassified` are decoder faults, not a
        // phrase the person can correct. A later `PhraseError` variant
        // lands here until it has its own sentence.
        _ => ERR_UNREADABLE.to_owned(),
    }
}

fn word_count_message() -> String {
    format!("Recovery phrase must be exactly {SHEKYL_MNEMONIC_WORD_COUNT} words")
}

/// The one-word-longer sentence. The two lengths come from
/// [`SHEKYL_MNEMONIC_WORD_COUNT`] and [`LEGACY_WORD_COUNT`].
fn legacy_phrase_message() -> String {
    format!(
        "Shekyl uses a {SHEKYL_MNEMONIC_WORD_COUNT}-word recovery phrase. \
         {LEGACY_WORD_COUNT}-word phrases from other wallets are not supported — Shekyl begins at its \
         own genesis and does not use legacy seed formats."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_words_decode_to_the_same_seed() {
        let (seed, backup) = generate().unwrap();
        let words = backup.into_mnemonic();
        assert_eq!(words.split_whitespace().count(), SHEKYL_MNEMONIC_WORD_COUNT);
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
            .take(SHEKYL_MNEMONIC_WORD_COUNT / 2)
            .collect::<Vec<_>>()
            .join(" ");
        let hex = "ab".repeat(SEED_BYTES);

        let err = seed_from_phrase("").unwrap_err();
        assert_eq!(err, ERR_EMPTY);

        // Spaces are no words. The decoder counts zero, the same class as empty.
        let err = seed_from_phrase("   ").unwrap_err();
        assert_eq!(err, ERR_EMPTY);

        let err = seed_from_phrase("café latté").unwrap_err();
        assert_eq!(err, word_count_message());
        assert!(!err.contains("caf"));

        let err = seed_from_phrase(&hex).unwrap_err();
        assert_eq!(err, word_count_message());
        assert!(!err.contains("ab"), "{err}");

        // A short BIP-39 word can sit inside a fixed sentence ("use" inside
        // "uses"). These refusals are the fixed sentences, so the assertion
        // is equality, not a search for a word drawn at random.
        let err = seed_from_phrase(&twelve).unwrap_err();
        assert_eq!(err, word_count_message());

        let legacy = format!("{words} extra");
        let err = seed_from_phrase(&legacy).unwrap_err();
        assert_eq!(err, legacy_phrase_message());
    }

    #[test]
    fn the_all_zero_phrase_names_a_bad_word_and_a_bad_checksum() {
        // The same vector `shekyl-crypto-pq`'s bip39 tests pin: 32 zero
        // bytes spell a phrase whose last word is "art". "ability" is in
        // the list and fails the checksum. "notaword" is not in the list.
        let words = mnemonic_from_entropy(&[0u8; SEED_BYTES]).unwrap();
        let mut parts: Vec<&str> = words.split_whitespace().collect();
        assert_eq!(parts.len(), SHEKYL_MNEMONIC_WORD_COUNT);
        assert_eq!(*parts.last().unwrap(), "art");

        parts[SHEKYL_MNEMONIC_WORD_COUNT - 1] = "notaword";
        let err = seed_from_phrase(&parts.join(" ")).unwrap_err();
        assert_eq!(err, ERR_UNKNOWN_WORD);
        assert!(!err.contains("notaword"));
        assert!(!err.contains("art"));

        parts[SHEKYL_MNEMONIC_WORD_COUNT - 1] = "ability";
        let err = seed_from_phrase(&parts.join(" ")).unwrap_err();
        assert_eq!(err, ERR_CHECKSUM);
        assert!(!err.contains("ability"));
        assert!(!err.contains("art"));
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
