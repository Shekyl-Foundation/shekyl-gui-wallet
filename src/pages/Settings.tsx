import { useState } from "react";
import { Globe, Server, Shield } from "lucide-react";

const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;

export default function Settings() {
  const [daemonUrl, setDaemonUrl] = useState("http://127.0.0.1:11029");
  const [network, setNetwork] = useState<string>("mainnet");

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
              onClick={() => setNetwork(n)}
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
          <label className="text-xs text-purple-300">RPC URL</label>
          <input
            type="text"
            className="input font-mono text-sm"
            value={daemonUrl}
            onChange={(e) => setDaemonUrl(e.target.value)}
          />
        </div>
        <button className="btn btn-secondary w-full text-xs">
          Test Connection
        </button>
      </div>

      {/* Security */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Shield className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">Security</h2>
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
