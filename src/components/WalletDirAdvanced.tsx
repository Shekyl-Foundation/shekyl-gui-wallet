import { useCallback, useEffect, useState } from "react";
import { FolderCog, FolderOpen, RotateCcw } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { useWallet } from "../context/useWallet";
import CollapsibleSection from "./CollapsibleSection";

/**
 * "Advanced" wallet-directory override used on the Create and Import
 * flows. Contracts out of the way by default so the happy-path user
 * never has to think about filesystem layout; advanced users can point
 * at a different directory without losing the default as a fallback.
 *
 * The missing-directory case is handled invisibly by the backend —
 * `set_wallet_dir` runs mkdir -p on the chosen path and the default
 * `init_wallet_rpc` does the same for the platform default on startup.
 */
export default function WalletDirAdvanced() {
  const { walletDir, setCustomWalletDir, resetWalletDir, refreshWalletDir } =
    useWallet();

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (walletDir == null) {
      refreshWalletDir().catch(() => {
        // Non-fatal: the header just shows a placeholder.
      });
    }
  }, [walletDir, refreshWalletDir]);

  const handleBrowse = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: walletDir ?? undefined,
      });
      if (typeof picked === "string" && picked.length > 0) {
        await setCustomWalletDir(picked);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [walletDir, setCustomWalletDir]);

  const handleReset = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      await resetWalletDir();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [resetWalletDir]);

  return (
    <CollapsibleSection
      icon={FolderCog}
      title="Advanced: wallet file location"
      className="rounded-lg border border-purple-800/60 bg-purple-900/40 p-4"
    >
      <p className="text-purple-200">
        By default, wallet files are stored in a Shekyl folder under your user
        profile. Advanced users can store them elsewhere — for example, on an
        encrypted volume. The directory is created automatically if it does
        not exist.
      </p>

      <div className="rounded-md bg-purple-900/60 px-3 py-2 font-mono text-[11px] text-gold-300 break-all">
        {walletDir ?? "Loading…"}
      </div>

      {error && (
        <div className="rounded-md border border-red-500/40 bg-red-900/30 px-3 py-2 text-red-200">
          {error}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={handleBrowse}
          disabled={busy}
          className="btn btn-ghost text-xs"
        >
          <FolderOpen className="h-3.5 w-3.5" />
          Choose folder…
        </button>
        <button
          type="button"
          onClick={handleReset}
          disabled={busy}
          className="btn btn-ghost text-xs"
        >
          <RotateCcw className="h-3.5 w-3.5" />
          Reset to default
        </button>
      </div>
    </CollapsibleSection>
  );
}
