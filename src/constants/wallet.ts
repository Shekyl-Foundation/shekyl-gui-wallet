/** BIP-39 English recovery phrase length for Shekyl genesis wallets. */
export const BIP39_RECOVERY_PHRASE_WORD_COUNT = 24;

/**
 * A testnet wallet backs up as its 32-byte raw seed in hex — the `seed`
 * `create_wallet` returns when `seed_language` is `"raw32"` — and restores
 * from the same string. Mainnet and stagenet use the phrase.
 */
export const RAW_SEED_HEX_LENGTH = 64;

export type SeedBackupShape = "phrase" | "raw32";

/**
 * Which backup a typed string is, by shape alone: 24 words, or 64 hex
 * characters. Which one the running network accepts is Rust's call
 * (`validate_seed_backup`); the page only refuses what is neither.
 */
export function seedBackupShape(text: string): SeedBackupShape | null {
  const trimmed = text.trim();
  if (new RegExp(`^[0-9a-fA-F]{${RAW_SEED_HEX_LENGTH}}$`).test(trimmed)) return "raw32";
  const words = trimmed.split(/\s+/).filter(Boolean).length;
  return words === BIP39_RECOVERY_PHRASE_WORD_COUNT ? "phrase" : null;
}
