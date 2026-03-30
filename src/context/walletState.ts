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
  refreshFiles: () => Promise<void>;
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
  refreshFiles: () => Promise.resolve(),
});
