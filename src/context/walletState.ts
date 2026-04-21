import { createContext } from "react";
import type {
  WalletPhase,
  WalletFileInfo,
  WalletInfo,
  CreateWalletResult,
} from "../types/wallet";

export interface WalletContextValue {
  phase: WalletPhase;
  walletFiles: WalletFileInfo[];
  walletName: string | null;
  walletAddress: string | null;
  rpcReady: boolean;
  error: string | null;

  openWallet: (filename: string, password: string) => Promise<WalletInfo>;
  createWallet: (
    name: string,
    password: string,
    language?: string,
  ) => Promise<CreateWalletResult>;
  importFromSeed: (
    name: string,
    seed: string,
    password: string,
    language?: string,
    restoreHeight?: number,
  ) => Promise<WalletInfo>;
  importFromKeys: (
    name: string,
    address: string,
    spendkey: string,
    viewkey: string,
    password: string,
    language?: string,
    restoreHeight?: number,
  ) => Promise<WalletInfo>;
  lockWallet: () => Promise<void>;
  setPhase: (phase: WalletPhase) => void;
  refreshFiles: () => Promise<WalletFileInfo[]>;

  /**
   * Currently active wallet directory, as a display string. Updated
   * whenever the Advanced directory picker or reset is used.
   */
  walletDir: string | null;
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

export const WalletContext = createContext<WalletContextValue>({
  phase: "loading",
  walletFiles: [],
  walletName: null,
  walletAddress: null,
  rpcReady: false,
  error: null,

  openWallet: () => Promise.reject("Not initialized"),
  createWallet: () => Promise.reject("Not initialized"),
  importFromSeed: () => Promise.reject("Not initialized"),
  importFromKeys: () => Promise.reject("Not initialized"),
  lockWallet: () => Promise.reject("Not initialized"),
  setPhase: () => {},
  refreshFiles: () => Promise.resolve([]),

  walletDir: null,
  setCustomWalletDir: () => Promise.reject("Not initialized"),
  resetWalletDir: () => Promise.reject("Not initialized"),
  refreshWalletDir: () => Promise.reject("Not initialized"),
});
