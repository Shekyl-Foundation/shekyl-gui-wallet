import { useEffect, useState, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  WalletPhase,
  WalletFileInfo,
  WalletInfo,
  CreateWalletResult,
} from "../types/wallet";
import { WalletContext } from "./walletState";

export function WalletProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<WalletPhase>("loading");
  const [walletFiles, setWalletFiles] = useState<WalletFileInfo[]>([]);
  const [walletName, setWalletName] = useState<string | null>(null);
  const [walletAddress, setWalletAddress] = useState<string | null>(null);
  const [rpcReady, setRpcReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [walletDir, setWalletDir] = useState<string | null>(null);

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
    const dir = await invoke<string>("get_wallet_dir");
    setWalletDir(dir);
    return dir;
  }, []);

  const setCustomWalletDir = useCallback(
    async (dir: string) => {
      const canonical = await invoke<string>("set_wallet_dir", { dir });
      setWalletDir(canonical);
      await refreshFiles();
      return canonical;
    },
    [refreshFiles],
  );

  const resetWalletDir = useCallback(async () => {
    const canonical = await invoke<string>("reset_wallet_dir");
    setWalletDir(canonical);
    await refreshFiles();
    return canonical;
  }, [refreshFiles]);

  useEffect(() => {
    let cancelled = false;

    async function bootstrap() {
      try {
        await invoke<boolean>("init_wallet_rpc");
        if (cancelled) return;
        setRpcReady(true);
        try {
          const dir = await invoke<string>("get_wallet_dir");
          if (!cancelled) setWalletDir(dir);
        } catch {
          // non-fatal; UI can request it later
        }
      } catch (e) {
        if (cancelled) return;
        setError(
          `Could not start wallet service: ${String(e)}. ` +
            "Make sure shekyl-wallet-rpc is installed and accessible.",
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
    async (name: string, password: string, language?: string) => {
      setError(null);
      const result = await invoke<CreateWalletResult>("create_wallet", {
        name,
        password,
        language: language ?? "English",
      });
      setWalletName(result.name);
      setWalletAddress(result.address);
      return result;
    },
    [],
  );

  const importFromSeed = useCallback(
    async (
      name: string,
      seed: string,
      password: string,
      language?: string,
      restoreHeight?: number,
    ) => {
      setError(null);
      const info = await invoke<WalletInfo>("import_wallet_from_seed", {
        name,
        seed,
        password,
        language: language ?? "English",
        restoreHeight: restoreHeight ?? 0,
      });
      setWalletName(info.name);
      setWalletAddress(info.address);
      setPhase("ready");
      return info;
    },
    [],
  );

  const importFromKeys = useCallback(
    async (
      name: string,
      address: string,
      spendkey: string,
      viewkey: string,
      password: string,
      language?: string,
      restoreHeight?: number,
    ) => {
      setError(null);
      const info = await invoke<WalletInfo>("import_wallet_from_keys", {
        name,
        address,
        spendkey,
        viewkey,
        password,
        language: language ?? "English",
        restoreHeight: restoreHeight ?? 0,
      });
      setWalletName(info.name);
      setWalletAddress(info.address);
      setPhase("ready");
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
        importFromSeed,
        importFromKeys,
        lockWallet,
        setPhase,
        refreshFiles,
        walletDir,
        setCustomWalletDir,
        resetWalletDir,
        refreshWalletDir,
      }}
    >
      {children}
    </WalletContext.Provider>
  );
}
