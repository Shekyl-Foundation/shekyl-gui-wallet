export interface WalletFileInfo {
  name: string;
  path: string;
  modified: number;
}

export interface WalletInfo {
  name: string;
  address: string;
  /** Phrase encoding the Engine chose: "BIP-39 English", or "raw32" on testnet. */
  seed_language: string;
  network: string;
}

export interface CreateWalletResult {
  name: string;
  address: string;
  seed: string;
  /** Encoding of `seed`: "BIP-39 English", or "raw32" on testnet. */
  seed_language: string;
  network: string;
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
  walletAddress: string | null;
  rpcReady: boolean;
  error: string | null;
}
