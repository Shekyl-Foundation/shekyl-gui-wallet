import { useEffect, useRef, useState, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  createdWalletFromWire,
  type WalletPhase,
  type WalletFileInfo,
  type OpenedWallet,
  type CreatedWalletWire,
} from "../types/wallet";
import { WalletContext, type WalletDirResponse } from "./walletState";

/** The resting phase a folder's wallets call for. */
function phaseFor(files: WalletFileInfo[]): WalletPhase {
  if (files.length === 0) return "no_wallet";
  if (files.length === 1) return "unlock";
  return "select_wallet";
}

export function WalletProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<WalletPhase>("loading");
  const [walletFiles, setWalletFiles] = useState<WalletFileInfo[]>([]);
  const [walletName, setWalletName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [walletDir, setWalletDir] = useState<string | null>(null);
  const [walletDirFallbackFrom, setWalletDirFallbackFrom] = useState<
    string | null
  >(null);

  // Every full read of the wallet folder is numbered: the start, a network
  // switch, a folder chosen or reset, a lock. Only the newest commits the
  // folder, its wallets and the phase they call for, so a slow read never
  // pairs one folder (or network) with another's wallets.
  const folderRead = useRef(0);

  /**
   * Reads the wallet folder in effect (the network's own unless a folder
   * was chosen), lists its wallets and, before a wallet is open, moves to
   * the phase they call for. The start runs it, and so do a change of
   * network before a wallet is open, a folder chosen or reset, and a lock:
   * each network keeps its wallets in its own folder, so the list and the
   * screen follow the folder in effect. Every pre-wallet phase shares one
   * route tree (`App`'s `PreWalletRoutes`), so a create or import form
   * under way keeps what was typed in it. An open wallet's phase is not the
   * list's to decide. A folder that cannot be read (a file in its place, a
   * refused permission) is said, never shown as no wallets.
   */
  const loadWallets = useCallback(async () => {
    const read = ++folderRead.current;
    const newest = () => read === folderRead.current;
    try {
      await invoke<void>("check_wallet_dir");
      const resp = await invoke<WalletDirResponse>("get_wallet_dir");
      const files = await invoke<WalletFileInfo[]>("check_wallet_files");
      if (!newest()) return;
      setWalletDir(resp.dir);
      setWalletDirFallbackFrom(resp.fallback_from ?? null);
      setWalletFiles(files);
      setError(null);
      setPhase((prev) => (prev === "ready" ? prev : phaseFor(files)));
    } catch (e) {
      if (!newest()) return;
      setWalletFiles([]);
      setError(
        `Could not start wallet service: ${String(e)}. ` +
          "The wallet runs inside this app — nothing separate to install. " +
          "This step only finds the wallet folder, so choose a different " +
          "folder in Settings if the current one can't be used.",
      );
      setPhase((prev) => (prev === "ready" ? prev : "no_wallet"));
    }
  }, []);

  /** The folder in effect alone, for a view that has none yet. */
  const refreshWalletDir = useCallback(async () => {
    const read = folderRead.current;
    const resp = await invoke<WalletDirResponse>("get_wallet_dir");
    // A full read begun since is newer; its folder stands.
    if (read === folderRead.current) {
      setWalletDir(resp.dir);
      setWalletDirFallbackFrom(resp.fallback_from ?? null);
    }
    return resp.dir;
  }, []);

  const setCustomWalletDir = useCallback(
    async (dir: string) => {
      const canonical = await invoke<string>("set_wallet_dir", { dir });
      await loadWallets();
      return canonical;
    },
    [loadWallets],
  );

  const resetWalletDir = useCallback(async () => {
    const canonical = await invoke<string>("reset_wallet_dir");
    await loadWallets();
    return canonical;
  }, [loadWallets]);

  useEffect(() => {
    loadWallets();
    // Leaving numbers out the read in flight.
    return () => {
      folderRead.current += 1;
    };
  }, [loadWallets]);

  const networkChanged = useCallback(() => loadWallets(), [loadWallets]);

  const openWallet = useCallback(
    async (filename: string, password: string) => {
      setError(null);
      const opened = await invoke<OpenedWallet>("open_wallet", {
        filename,
        password,
      });
      setWalletName(opened.wallet.name);
      setPhase("ready");
      return opened;
    },
    [],
  );

  const createWallet = useCallback(
    async (name: string, password: string) => {
      setError(null);
      const created = createdWalletFromWire(
        await invoke<CreatedWalletWire>("create_wallet", {
          name,
          password,
        }),
      );
      setWalletName(created.wallet.name);
      return created;
    },
    [],
  );

  const restoreWallet = useCallback(
    async (name: string, password: string, mnemonic: string, restoreHeight?: number) => {
      setError(null);
      const opened = await invoke<OpenedWallet>("restore_wallet", {
        name,
        password,
        mnemonic,
        restoreHeight: restoreHeight ?? 0,
      });
      setWalletName(opened.wallet.name);
      // The phase stays put, as with createWallet: the Import page owns the
      // transition (show completion, navigate off /import, then "ready"),
      // because the ready-phase routes have no /import entry — flipping here
      // would unmount the page mid-flow into a blank route.
      return opened;
    },
    [],
  );

  const lockWallet = useCallback(async () => {
    try {
      await invoke<boolean>("close_wallet");
    } catch {
      // ignore close errors
    }
    setWalletName(null);
    // Out of the open wallet's phase, then the folder's list decides.
    setPhase("loading");
    await loadWallets();
  }, [loadWallets]);

  return (
    <WalletContext.Provider
      value={{
        phase,
        walletFiles,
        walletName,
        error,
        openWallet,
        createWallet,
        restoreWallet,
        lockWallet,
        setPhase,
        walletDir,
        walletDirFallbackFrom,
        setCustomWalletDir,
        resetWalletDir,
        refreshWalletDir,
        networkChanged,
      }}
    >
      {children}
    </WalletContext.Provider>
  );
}
