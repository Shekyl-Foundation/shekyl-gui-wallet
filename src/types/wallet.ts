import { BIP39_RECOVERY_PHRASE_WORD_COUNT, RAW_SEED_HEX_LENGTH } from "../constants/wallet";

export interface WalletFileInfo {
  name: string;
  path: string;
  modified: number;
}

/** The contract's `CapabilityMode`. `FULL` is the only capability. */
export type CapabilityMode = "FULL";

export type WalletNetwork = "MAINNET" | "TESTNET" | "STAGENET";

/** The contract's `WalletHandle`: returned by every lifecycle call that leaves a wallet open. */
export interface WalletHandle {
  name: string;
  capability: CapabilityMode;
  network: WalletNetwork;
  /**
   * Present when a restore used a floor above genesis, or when open rebuilt
   * the ledger (including a zero floor). Absent on create, on a genesis
   * restore, and on a cache-hit open.
   */
  restore_height_hint?: number;
}

/** `open_wallet` / `restore_wallet`. The address comes from `get_primary_address`. */
export interface OpenedWallet {
  wallet: WalletHandle;
}

/**
 * Wire object `create_wallet` returns. The contract carries exactly one of
 * `mnemonic` and `raw_seed_hex`. [`createdWalletFromWire`] is what the app holds.
 */
export interface CreatedWalletWire {
  wallet: WalletHandle;
  mnemonic?: string;
  raw_seed_hex?: string;
}

/**
 * `create_wallet` after the wire object has been checked. Exactly one backup
 * arm: a 24-word phrase, or the testnet 64-character hex seed.
 */
export type CreatedWallet =
  | { wallet: WalletHandle; encoding: "mnemonic"; mnemonic: string }
  | { wallet: WalletHandle; encoding: "raw_seed_hex"; raw_seed_hex: string };

/** Shown when create succeeded but the backup was not one contract encoding. */
export const CREATED_WALLET_BACKUP_UNUSABLE =
  "The new wallet did not return a usable backup. Do not continue — without the backup the funds cannot be recovered.";

function nonempty(value: string | undefined): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length === 0 ? undefined : trimmed;
}

function isRawSeedHex(value: string): boolean {
  return value.length === RAW_SEED_HEX_LENGTH && /^[0-9a-fA-F]+$/.test(value);
}

function isRecoveryPhrase(value: string): boolean {
  return value.split(" ").filter(Boolean).length === BIP39_RECOVERY_PHRASE_WORD_COUNT;
}

/**
 * Accept the contract object only when exactly one backup is present and
 * well-formed. Both, neither, an empty string, a short phrase, and a
 * non-hex seed are refused.
 */
export function createdWalletFromWire(wire: CreatedWalletWire): CreatedWallet {
  const mnemonic = nonempty(wire.mnemonic);
  const rawSeedHex = nonempty(wire.raw_seed_hex);
  if (mnemonic !== undefined && rawSeedHex !== undefined) {
    throw new Error(CREATED_WALLET_BACKUP_UNUSABLE);
  }
  if (mnemonic !== undefined && isRecoveryPhrase(mnemonic)) {
    return { wallet: wire.wallet, encoding: "mnemonic", mnemonic };
  }
  if (rawSeedHex !== undefined && mnemonic === undefined && isRawSeedHex(rawSeedHex)) {
    return {
      wallet: wire.wallet,
      encoding: "raw_seed_hex",
      raw_seed_hex: rawSeedHex.toLowerCase(),
    };
  }
  throw new Error(CREATED_WALLET_BACKUP_UNUSABLE);
}

/** The one backup string a created wallet handed out. */
export function seedBackupOf(created: CreatedWallet): string {
  return created.encoding === "mnemonic" ? created.mnemonic : created.raw_seed_hex;
}

export type WalletPhase =
  | "loading"
  | "no_wallet"
  | "select_wallet"
  | "unlock"
  | "creating"
  | "importing"
  | "ready";

export interface WalletState {
  phase: WalletPhase;
  walletFiles: WalletFileInfo[];
  walletName: string | null;
  error: string | null;
}

/** `get_primary_address`: one address, no index — Shekyl has no subaddresses. */
export interface PrimaryAddress {
  address: string;
}
