import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import NetworkBadge from "./NetworkBadge";

interface WalletStatus {
  connected: boolean;
  wallet_open: boolean;
  wallet_name: string | null;
  daemon_address: string | null;
  network: string;
  synced: boolean;
  sync_height: number;
  daemon_height: number;
}

export default function Header() {
  const [status, setStatus] = useState<WalletStatus | null>(null);

  useEffect(() => {
    invoke<WalletStatus>("get_wallet_status").then(setStatus).catch(() => {});
  }, []);

  return (
    <header className="flex items-center justify-between border-b border-purple-700/50 bg-purple-900/60 px-6 py-3">
      <div className="flex items-center gap-3">
        <NetworkBadge network={status?.network ?? "mainnet"} />
        {status?.wallet_open && (
          <span className="text-sm text-purple-200">
            {status.wallet_name}
          </span>
        )}
      </div>

      <div className="flex items-center gap-4 text-xs text-purple-300">
        {status?.connected ? (
          <>
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-emerald-400" />
              Synced
            </span>
            <span>
              Block {status.sync_height.toLocaleString()}
            </span>
          </>
        ) : (
          <span className="flex items-center gap-1.5">
            <span className="h-2 w-2 rounded-full bg-amber-400" />
            Not connected
          </span>
        )}
      </div>
    </header>
  );
}
