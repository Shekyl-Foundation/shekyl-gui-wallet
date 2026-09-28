import { createContext } from "react";
import type {
  WalletPhase,
  WalletFileInfo,
  OpenedWallet,
  CreatedWallet,
} from "../types/wallet";

export interface WalletContextValue {
  phase: WalletPhase;
  walletFiles: WalletFileInfo[];
  walletName: string | null;
  error: string | null;

  openWallet: (filename: string, password: string) => Promise<OpenedWallet>;
  createWallet: (name: string, password: string) => Promise<CreatedWallet>;
  /**
   * The contract's `restore_wallet(name, password, mnemonic, restore_height)`.
   * `mnemonic` is the seed backup in the network's encoding.
   */
  restoreWallet: (
    name: string,
    password: string,
    mnemonic: string,
    restoreHeight?: number,
  ) => Promise<OpenedWallet>;
  lockWallet: () => Promise<void>;
  setPhase: (phase: WalletPhase) => void;
  refreshFiles: () => Promise<WalletFileInfo[]>;

  /**
   * Currently active wallet directory, as a display string. Updated
   * whenever the Advanced directory picker or reset is used.
   */
  walletDir: string | null;
  /**
   * Set when the persisted custom wallet directory was unreachable at
   * startup (permission denied, target is a file, etc.) and the app
   * silently fell back to the platform default. The UI uses this to
   * show a "your custom location is unavailable" banner; cleared once
   * the user picks a new (working) directory or resets to default.
   */
  walletDirFallbackFrom: string | null;
  /**
   * Override the wallet directory. Creates the directory if it does not
   * exist (mkdir -p semantics). Returns the canonical display path.
   */
  setCustomWalletDir: (dir: string) => Promise<string>;
  /** Reset to the platform default (~/.shekyl/wallets, etc.). */
  resetWalletDir: () => Promise<string>;
  /** Re-read the current directory from the backend into `walletDir`. */
  refreshWalletDir: () => Promise<string>;
}

/**
 * Tauri command response for `get_wallet_dir`. The backend now returns
 * a struct so it can surface the soft "fell back from override"
 * warning alongside the active directory. `fallback_from` is omitted
 * (undefined) when no fallback occurred.
 */
export interface WalletDirResponse {
  dir: string;
  fallback_from?: string;
}

export const WalletContext = createContext<WalletContextValue>({
  phase: "loading",
  walletFiles: [],
  walletName: null,
  error: null,

  openWallet: () => Promise.reject("Not initialized"),
  createWallet: () => Promise.reject("Not initialized"),
  restoreWallet: () => Promise.reject("Not initialized"),
  lockWallet: () => Promise.reject("Not initialized"),
  setPhase: () => {},
  refreshFiles: () => Promise.resolve([]),

  walletDir: null,
  walletDirFallbackFrom: null,
  setCustomWalletDir: () => Promise.reject("Not initialized"),
  resetWalletDir: () => Promise.reject("Not initialized"),
  refreshWalletDir: () => Promise.reject("Not initialized"),
});
