import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import NodeNotice from "./NodeNotice";
import NodeAddress from "./NodeAddress";
import type { DaemonConnection } from "../types/node";
import { useWallet } from "../context/useWallet";

/** The networks a person can choose before a wallet is open. */
const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;

/** How often the bar reads the connection in effect. */
const CONNECTION_POLL_MS = 2000;

/**
 * The wallet's node, before a wallet is open: its notice (a node of yours
 * found, a failed start, a held data directory, a failed check, a service)
 * with the acts it offers; the network, which a person can change here;
 * and the node address, for a person who reads another node than this
 * computer's. A wallet is created, opened or restored on that network
 * against that node, so none of them may wait for a wallet to be open
 * (Settings is reached only with one): a service with no view listener,
 * above all, refuses every open until another node is named.
 */
export default function PreWalletNode() {
  const [connection, setConnection] = useState<DaemonConnection | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingNode, setEditingNode] = useState(false);
  const { networkChanged } = useWallet();

  const load = useCallback(async () => {
    try {
      const current = await invoke<DaemonConnection | undefined>(
        "daemon_connection_disclosures",
      );
      if (current) setConnection(current);
    } catch {
      // The notice below says what the node is doing.
    }
  }, []);

  useEffect(() => {
    const first = setTimeout(load, 0);
    const timer = setInterval(load, CONNECTION_POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [load]);

  // No address rides the switch: a typed address names a node of one
  // network, so the wallet is pointed back at this computer's node, which
  // follows the network. Each network keeps its wallets in its own folder,
  // so the list under the bar is read again for the new one.
  async function choose(net: string) {
    setSwitching(true);
    setError(null);
    try {
      setConnection(
        await invoke<DaemonConnection>("set_daemon_connection", { network: net }),
      );
      await networkChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setSwitching(false);
    }
  }

  const network = connection?.network ?? null;
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
        <span className="ml-auto">
          {connection?.url ? `Node: ${connection.url}` : "Node: this computer’s"}
        </span>
        <button
          type="button"
          aria-expanded={editingNode}
          onClick={() => setEditingNode((open) => !open)}
          className="rounded border border-purple-600 px-2 py-0.5 font-semibold hover:border-purple-500"
        >
          {editingNode ? "Done" : "Change node"}
        </button>
        {error && <span className="text-amber-200">{error}</span>}
      </div>
      {editingNode && (
        <div className="mx-auto max-w-lg px-4 pb-3">
          <NodeAddress connection={connection} onApplied={setConnection} />
        </div>
      )}
    </div>
  );
}
