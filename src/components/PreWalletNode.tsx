import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import NodeNotice from "./NodeNotice";
import type { NodeStatus } from "../types/node";

/** The networks a person can choose before a wallet is open. */
const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;

/** How often the bar reads the node's network. */
const NETWORK_POLL_MS = 2000;

/**
 * The wallet's own node, before a wallet is open: its notice (a node of
 * yours found, a failed start, a held data directory, a failed check) with
 * the acts it offers, and the network the node runs, which a person can
 * change here. A wallet is created, opened or restored on that network, and
 * its node must be running for any of them, so neither may wait for a
 * wallet to be open (Settings is reached only with one).
 */
export default function PreWalletNode() {
  const [network, setNetwork] = useState<string | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const status = await invoke<NodeStatus | undefined>("daemon_status");
      if (status) setNetwork(status.network);
    } catch {
      // The notice below says what the node is doing.
    }
  }, []);

  useEffect(() => {
    const first = setTimeout(load, 0);
    const timer = setInterval(load, NETWORK_POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [load]);

  async function choose(net: string) {
    setSwitching(true);
    setError(null);
    try {
      await invoke("set_daemon_connection", { network: net });
      setNetwork(net);
    } catch (e) {
      setError(String(e));
    } finally {
      setSwitching(false);
    }
  }

  return (
    <div className="border-b border-purple-700/50 bg-purple-900/60">
      <NodeNotice />
      <div className="flex flex-wrap items-center gap-2 px-4 py-2 text-xs text-purple-300">
        <span>Network</span>
        {NETWORKS.map((n) => (
          <button
            key={n}
            type="button"
            onClick={() => choose(n)}
            disabled={switching || network === n}
            aria-pressed={network === n}
            className={`rounded border px-2 py-0.5 font-semibold uppercase tracking-wider ${
              network === n
                ? "border-gold-500 bg-gold-500/15 text-gold-400"
                : "border-purple-600 text-purple-300 hover:border-purple-500"
            }`}
          >
            {n}
          </button>
        ))}
        {error && <span className="text-amber-200">{error}</span>}
      </div>
    </div>
  );
}
