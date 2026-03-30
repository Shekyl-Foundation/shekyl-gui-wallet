import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, Server, Shield, ShieldCheck } from "lucide-react";
import { useDaemon } from "../context/useDaemon";

const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;

const DEFAULT_PORTS: Record<string, number> = {
  mainnet: 11029,
  testnet: 12029,
  stagenet: 13029,
};

export default function Settings() {
  const { health, pqc, refresh } = useDaemon();
  const [network, setNetwork] = useState<string>(health?.network ?? "mainnet");
  const [daemonUrl, setDaemonUrl] = useState(
    `http://127.0.0.1:${DEFAULT_PORTS[network]}/json_rpc`,
  );
  const [saving, setSaving] = useState(false);

  async function handleNetworkSwitch(net: string) {
    setNetwork(net);
    const url = `http://127.0.0.1:${DEFAULT_PORTS[net]}/json_rpc`;
    setDaemonUrl(url);
    setSaving(true);
    try {
      await invoke("set_daemon_connection", { network: net, url });
      refresh();
    } catch {
      // ignore
    } finally {
      setSaving(false);
    }
  }

  async function handleSaveUrl() {
    setSaving(true);
    try {
      await invoke("set_daemon_connection", { network, url: daemonUrl });
      refresh();
    } catch {
      // ignore
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Settings</h1>

      {/* Network */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Globe className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">Network</h2>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {NETWORKS.map((n) => (
            <button
              key={n}
              onClick={() => handleNetworkSwitch(n)}
              disabled={saving}
              className={`rounded-lg border px-3 py-2 text-xs font-semibold uppercase tracking-wider transition-colors ${
                network === n
                  ? "border-gold-500 bg-gold-500/15 text-gold-400"
                  : "border-purple-600 text-purple-300 hover:border-purple-500"
              }`}
            >
              {n}
            </button>
          ))}
        </div>
      </div>

      {/* Daemon */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Server className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Daemon Connection
          </h2>
        </div>
        <div className="space-y-2">
          <label className="text-xs text-purple-300">JSON-RPC URL</label>
          <input
            type="text"
            className="input font-mono text-sm"
            value={daemonUrl}
            onChange={(e) => setDaemonUrl(e.target.value)}
          />
        </div>
        <button
          onClick={handleSaveUrl}
          disabled={saving}
          className="btn btn-secondary w-full text-xs"
        >
          {saving ? "Connecting..." : "Save & Reconnect"}
        </button>
        {health && (
          <p className="text-center text-[10px] text-emerald-400">
            Connected — Block {health.height.toLocaleString()} — {health.version}
          </p>
        )}
      </div>

      {/* PQC Security */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <ShieldCheck className="h-4 w-4 text-emerald-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Post-Quantum Security
          </h2>
        </div>
        {pqc ? (
          <div className="space-y-3 text-xs">
            <div className="flex justify-between">
              <span className="text-purple-300">Signature scheme</span>
              <span className="font-semibold text-white">
                {pqc.scheme}: {pqc.classical} + {pqc.post_quantum}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-purple-300">Transaction version</span>
              <span className="font-mono text-white">v{pqc.tx_version}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-purple-300">Status</span>
              <span className="text-emerald-400">{pqc.description}</span>
            </div>
            <div className="rounded-lg bg-purple-800/40 p-3 text-purple-300">
              <p className="font-semibold text-purple-200">V4 Roadmap</p>
              <p className="mt-1 leading-relaxed">
                Lattice-based ring signatures and KEM stealth addresses
                (X25519 + ML-KEM-768) are planned for V4, providing
                full post-quantum privacy for transaction inputs and
                one-time addresses.
              </p>
            </div>
          </div>
        ) : (
          <p className="text-xs text-purple-300">Loading PQC status...</p>
        )}
      </div>

      {/* Wallet Security */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Shield className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">Wallet</h2>
        </div>
        <button className="btn btn-secondary w-full text-xs">
          Change Wallet Password
        </button>
        <button className="btn btn-danger w-full text-xs">
          Close Wallet
        </button>
      </div>
    </div>
  );
}
