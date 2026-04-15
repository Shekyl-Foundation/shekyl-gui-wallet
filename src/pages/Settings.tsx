import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, Server, Shield, ShieldCheck, Settings2 } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import SecurityPanel from "../components/SecurityPanel";

interface DaemonSettings {
  keep_running_on_exit: boolean;
  data_dir: string | null;
  rpc_port: number;
}

interface DaemonStatusInfo {
  mode: "managed" | "external" | "unavailable";
  ready: boolean;
  height: number;
}

const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;

const DEFAULT_PORTS: Record<string, number> = {
  mainnet: 11029,
  testnet: 12029,
  stagenet: 13029,
};

export default function Settings() {
  const { health, refresh } = useDaemon();
  const [network, setNetwork] = useState<string>(health?.network ?? "mainnet");
  const [daemonUrl, setDaemonUrl] = useState(
    `http://127.0.0.1:${DEFAULT_PORTS[network]}/json_rpc`,
  );
  const [saving, setSaving] = useState(false);
  const [daemonSettings, setDaemonSettings] = useState<DaemonSettings | null>(null);
  const [daemonStatus, setDaemonStatus] = useState<DaemonStatusInfo | null>(null);

  const loadDaemonInfo = useCallback(async () => {
    try {
      const [settings, status] = await Promise.all([
        invoke<DaemonSettings>("get_daemon_settings"),
        invoke<DaemonStatusInfo>("daemon_status"),
      ]);
      setDaemonSettings(settings);
      setDaemonStatus(status);
    } catch {
      // daemon commands not available yet
    }
  }, []);

  useEffect(() => { loadDaemonInfo(); }, [loadDaemonInfo]);

  async function handleKeepRunningToggle(value: boolean) {
    try {
      const updated = await invoke<DaemonSettings>("set_daemon_settings", {
        keepRunningOnExit: value,
      });
      setDaemonSettings(updated);
    } catch {
      // ignore
    }
  }

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

      {/* Daemon Management */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Settings2 className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Advanced Daemon
          </h2>
        </div>

        {daemonStatus && (
          <div className="flex items-center gap-2 text-xs">
            <span
              className={`inline-block h-2 w-2 rounded-full ${
                daemonStatus.ready ? "bg-emerald-400" : "bg-red-400"
              }`}
            />
            <span className="text-purple-300">
              {daemonStatus.mode === "managed"
                ? "Managed daemon"
                : daemonStatus.mode === "external"
                  ? "External daemon"
                  : "No daemon"}
              {daemonStatus.ready ? " (connected)" : " (offline)"}
            </span>
          </div>
        )}

        {daemonSettings && (
          <label className="flex items-center justify-between gap-3">
            <span className="text-xs text-purple-300">
              Keep daemon running after wallet closes
            </span>
            <button
              role="switch"
              aria-checked={daemonSettings.keep_running_on_exit}
              onClick={() =>
                handleKeepRunningToggle(!daemonSettings.keep_running_on_exit)
              }
              className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors ${
                daemonSettings.keep_running_on_exit
                  ? "bg-gold-500"
                  : "bg-purple-700"
              }`}
            >
              <span
                className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                  daemonSettings.keep_running_on_exit
                    ? "translate-x-4"
                    : "translate-x-0"
                }`}
              />
            </button>
          </label>
        )}
      </div>

      {/* Security Overview */}
      <div className="space-y-2">
        <div className="flex items-center gap-2 px-1">
          <ShieldCheck className="h-4 w-4 text-emerald-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Transaction Security
          </h2>
        </div>
        <SecurityPanel />
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
