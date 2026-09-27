import { useEffect, useState, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  WalletPhase,
  WalletFileInfo,
  WalletInfo,
  CreateWalletResult,
} from "../types/wallet";
import { WalletContext, type WalletDirResponse } from "./walletState";

export function WalletProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<WalletPhase>("loading");
  const [walletFiles, setWalletFiles] = useState<WalletFileInfo[]>([]);
  const [walletName, setWalletName] = useState<string | null>(null);
  const [walletAddress, setWalletAddress] = useState<string | null>(null);
  const [rpcReady, setRpcReady] = useState(false);
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

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        await invoke<void>("ensure_wallet_dir");
        if (cancelled) return;
        setRpcReady(true);
        try {
          const resp = await invoke<WalletDirResponse>("get_wallet_dir");
          if (!cancelled) {
            setWalletDir(resp.dir);
            setWalletDirFallbackFrom(resp.fallback_from ?? null);
          }
        } catch {
          // non-fatal; UI can request it later
        }
      } catch (e) {
        if (cancelled) return;
        setError(
          `Could not start wallet service: ${String(e)}. ` +
            "The wallet runs inside this app — nothing separate to install. " +
            "This step only prepares the wallet folder, so choose a different " +
            "folder in Settings if the current one can't be used.",
        );
        setPhase("no_wallet");
        return;
      }

      const files = await refreshFiles();
      if (cancelled) return;

      if (files.length === 0) {
        setPhase("no_wallet");
      } else if (files.length === 1) {
        setPhase("unlock");
      } else {
        setPhase("select_wallet");
      }
    }

    bootstrap();
    return () => {
      cancelled = true;
    };
  }, [refreshFiles]);

  const openWallet = useCallback(
    async (filename: string, password: string) => {
      setError(null);
      const info = await invoke<WalletInfo>("open_wallet", {
        filename,
        password,
      });
      setWalletName(info.name);
      setWalletAddress(info.address);
      setPhase("ready");
      return info;
    },
    [],
  );

  const createWallet = useCallback(
    async (name: string, password: string) => {
      setError(null);
      const result = await invoke<CreateWalletResult>("create_wallet", {
        name,
        password,
      });
      setWalletName(result.name);
      setWalletAddress(result.address);
      return result;
    },
    [],
  );

  const restoreWallet = useCallback(
    async (name: string, mnemonic: string, password: string, restoreHeight?: number) => {
      setError(null);
      const info = await invoke<WalletInfo>("restore_wallet", {
        name,
        password,
        mnemonic,
        restoreHeight: restoreHeight ?? 0,
      });
      setWalletName(info.name);
      setWalletAddress(info.address);
      // The phase stays put, as with createWallet: the Import page owns the
      // transition (show completion, navigate off /import, then "ready"),
      // because the ready-phase routes have no /import entry — flipping here
      // would unmount the page mid-flow into a blank route.
      return info;
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
    setWalletAddress(null);
    const files = await refreshFiles();
    setPhase(files.length > 0 ? "unlock" : "no_wallet");
  }, [refreshFiles]);

  return (
    <WalletContext.Provider
      value={{
        phase,
        walletFiles,
        walletName,
        walletAddress,
        rpcReady,
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
      }}
    >
      {children}
    </WalletContext.Provider>
  );
}
