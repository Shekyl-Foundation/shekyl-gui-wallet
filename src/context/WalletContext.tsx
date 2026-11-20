import { useEffect, useState, useCallback, type ReactNode } from "react";
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

  const refreshFiles = useCallback(async () => {
    try {
      const files = await invoke<WalletFileInfo[]>("check_wallet_files");
      setWalletFiles(files);
      return files;
    } catch {
      setWalletFiles([]);
      return [];
    }
  }, []);

  const refreshWalletDir = useCallback(async () => {
    const resp = await invoke<WalletDirResponse>("get_wallet_dir");
    setWalletDir(resp.dir);
    setWalletDirFallbackFrom(resp.fallback_from ?? null);
    return resp.dir;
  }, []);

  const setCustomWalletDir = useCallback(
    async (dir: string) => {
      const canonical = await invoke<string>("set_wallet_dir", { dir });
      setWalletDir(canonical);
      // An explicit successful choice clears any stale fallback warning.
      setWalletDirFallbackFrom(null);
      await refreshFiles();
      return canonical;
    },
    [refreshFiles],
  );

  const resetWalletDir = useCallback(async () => {
    const canonical = await invoke<string>("reset_wallet_dir");
    setWalletDir(canonical);
    setWalletDirFallbackFrom(null);
    await refreshFiles();
    return canonical;
  }, [refreshFiles]);

  /**
   * Reads the wallet folder in effect (the network's own unless a folder
   * was chosen), lists its wallets and, before a wallet is open, moves to
   * the phase they call for. The start runs it, and so does a change of
   * network before a wallet is open: each network keeps its wallets in its
   * own folder, so the list and the screen follow the network. Every
   * pre-wallet phase shares one route tree (`App`'s `PreWalletRoutes`), so
   * a create or import form under way keeps what was typed in it. An open
   * wallet's phase is not the list's to decide.
   */
  const loadWallets = useCallback(
    async (cancelled: () => boolean) => {
      try {
        await invoke<void>("ensure_wallet_dir");
        if (cancelled()) return;
        try {
          const resp = await invoke<WalletDirResponse>("get_wallet_dir");
          if (!cancelled()) {
            setWalletDir(resp.dir);
            setWalletDirFallbackFrom(resp.fallback_from ?? null);
          }
        } catch {
          // non-fatal; UI can request it later
        }
      } catch (e) {
        if (cancelled()) return;
        setError(
          `Could not start wallet service: ${String(e)}. ` +
            "The wallet runs inside this app — nothing separate to install. " +
            "This step only prepares the wallet folder, so choose a different " +
            "folder in Settings if the current one can't be used.",
        );
        setPhase((prev) => (prev === "ready" ? prev : "no_wallet"));
        return;
      }

      const files = await refreshFiles();
      if (cancelled()) return;
      setPhase((prev) => (prev === "ready" ? prev : phaseFor(files)));
    },
    [refreshFiles],
  );

  useEffect(() => {
    let cancelled = false;
    loadWallets(() => cancelled);
    return () => {
      cancelled = true;
    };
  }, [loadWallets]);

  const networkChanged = useCallback(async () => {
    setError(null);
    await loadWallets(() => false);
  }, [loadWallets]);

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
    const files = await refreshFiles();
    setPhase(files.length > 0 ? "unlock" : "no_wallet");
  }, [refreshFiles]);

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
        refreshFiles,
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
