/**
 * BIP-39 English recovery phrase length: a wallet's 32-byte seed is shown as
 * 24 words, the one backup encoding on every network.
 */
export const BIP39_RECOVERY_PHRASE_WORD_COUNT = 24;

/**
 * True when a typed backup has the phrase's shape: exactly 24 words. Whether
 * the words decode is Rust's call (`seed_from_phrase`); the page only
 * refuses what cannot be a phrase.
 */
export function hasRecoveryPhraseShape(text: string): boolean {
  return text.trim().split(/\s+/).filter(Boolean).length === BIP39_RECOVERY_PHRASE_WORD_COUNT;
}
