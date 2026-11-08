import { hasRecoveryPhraseShape } from "../constants/wallet";

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
 * Why `open_wallet` refused, as the backend types it (`OpenRefusal` in
 * `src-tauri/src/engine_errors.rs`). The page branches on `kind`, never on
 * the words.
 */
export type OpenRefusal =
  | { kind: "earlier_release"; message: string }
  | { kind: "wrong_password"; message: string }
  | { kind: "other"; message: string };

/**
 * A rejected `open_wallet` as an [`OpenRefusal`]. Anything without a known
 * `kind` is `other`, keeping its `message` when it has one (a transport
 * error is an object with a message and no kind).
 */
export function asOpenRefusal(e: unknown): OpenRefusal {
  if (typeof e === "object" && e !== null) {
    const { kind, message } = e as { kind?: unknown; message?: unknown };
    if (typeof message === "string") {
      if (kind === "earlier_release" || kind === "wrong_password" || kind === "other") {
        return { kind, message };
      }
      return { kind: "other", message };
    }
  }
  return { kind: "other", message: String(e) };
}

/**
 * Wire object `create_wallet` returns. The contract carries the backup once,
 * as `mnemonic` — the seed's 24 BIP-39 words, on every network.
 * [`createdWalletFromWire`] is what the app holds.
 */
export interface CreatedWalletWire {
  wallet: WalletHandle;
  mnemonic?: string;
}

/** `create_wallet` after the wire object has been checked: the 24-word phrase. */
export interface CreatedWallet {
  wallet: WalletHandle;
  mnemonic: string;
}

/** Shown when create succeeded but the backup was not a 24-word phrase. */
export const CREATED_WALLET_BACKUP_UNUSABLE =
  "The new wallet did not return a usable backup. Do not continue — without the backup the funds cannot be recovered.";

/**
 * Accept the contract object only when its backup is a 24-word phrase. A
 * missing, empty or short phrase is a contract violation, refused whatever
 * the handle's network.
 */
export function createdWalletFromWire(wire: CreatedWalletWire): CreatedWallet {
  const mnemonic = wire.mnemonic?.trim() ?? "";
  if (!hasRecoveryPhraseShape(mnemonic)) {
    throw new Error(CREATED_WALLET_BACKUP_UNUSABLE);
  }
  return { wallet: wire.wallet, mnemonic };
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
