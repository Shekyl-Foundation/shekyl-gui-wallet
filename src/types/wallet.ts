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
  /** The rescan floor the wallet file carries; absent when it scans from genesis. */
  restore_height_hint?: number;
}

/** `open_wallet` / `restore_wallet`. The address comes from `get_primary_address`. */
export interface OpenedWallet {
  wallet: WalletHandle;
}

/**
 * `create_wallet`. The backup is returned exactly once, in the network's
 * encoding: `mnemonic` (24 words) on mainnet/stagenet, `raw_seed_hex` (64 hex)
 * on testnet — never both. Persist it before discarding the response.
 */
export interface CreatedWallet {
  wallet: WalletHandle;
  mnemonic?: string;
  raw_seed_hex?: string;
}

/** The one backup string a created wallet handed out, whichever encoding it used. */
export function seedBackupOf(created: CreatedWallet): string {
  return created.mnemonic ?? created.raw_seed_hex ?? "";
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
