import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, Server, Shield, ShieldCheck, Settings2 } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { useWallet } from "../context/useWallet";
import SecurityPanel from "../components/SecurityPanel";
import WalletCare from "../components/WalletCare";
import { BLOCK_NUMBER_TIP } from "../lib/format";
import NodeAddress from "../components/NodeAddress";
import type { DaemonConnection, NodeState, NodeStatus } from "../types/node";
import type { TorFix } from "../types/tor";
import TorSettings from "../components/TorSettings";

/** How often Settings reads the node's state while it is open. */
const SETTINGS_POLL_MS = 2000;

interface DaemonSettings {
  data_dir: string | null;
  /** The Tor fix in force; `null` is the node's defaults with the bundled Tor. */
  tor_fix: TorFix | null;
  /** Whether the node accepts inbound connections over clearnet. Off by default. */
  clearnet_inbound: boolean;
}

/** The wallet's own node, in a few words, for the Settings line. */
const NODE_STATE_WORDS: Record<NodeState, string> = {
  idle: "The wallet’s node has not started",
  starting: "The wallet’s node is starting",
  running: "The wallet’s node is running",
  stopping: "A node is stopping",
  waiting: "The data directory is still held; the wallet’s node is waiting for it",
  found: "Another node of yours is running; the wallet’s own is not",
  service: "This computer’s node runs as a service; the wallet starts none beside it",
  refused: "The node’s local address failed its check",
  failed: "The wallet’s node is not running",
  unavailable: "This build carries no node",
};

const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;


export default function Settings() {
  const { health, refresh } = useDaemon();
  // Closing the wallet is how the node or the network it uses is changed:
  // an open wallet keeps both.
  const { lockWallet } = useWallet();
  // The connection in effect, as the backend keeps it: the node address a
  // person typed (none for this computer's node) and the network the
  // wallet is on. The backend is the one source for both; the node's
  // health is not, since it is unread while the node is away and follows
  // a switch only once the node has restarted.
  const [connection, setConnection] = useState<DaemonConnection | null>(null);
  const network = connection?.network ?? null;
  // Each read of the node's state is numbered; an answer older than the
  // last one shown is dropped, so a slow read never paints over a newer one.
  const readGeneration = useRef(0);
  const [networkError, setNetworkError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [daemonSettings, setDaemonSettings] = useState<DaemonSettings | null>(null);
  const [daemonStatus, setDaemonStatus] = useState<NodeStatus | null>(null);
  const [keepRunningAnswer, setKeepRunningAnswer] = useState<string | null>(null);
  const [inboundBusy, setInboundBusy] = useState(false);
  const [inboundError, setInboundError] = useState<string | null>(null);

  // The node's state, its settings, its Tor read and the connection, read
  // together while the page is open: a node that recovers, or a target
  // that changes, shows here without leaving the page. Only the newest
  // read is shown.
  const loadDaemonInfo = useCallback(async () => {
    const generation = ++readGeneration.current;
    try {
      // A wallet that starts pointed at somebody else's daemon says so on
      // sight, not only in the session where the URL was typed.
      const current = await invoke<DaemonConnection>(
        "daemon_connection_disclosures",
      );
      // The URL as well as its warnings: the field is what "Save &
      // Reconnect" writes back, so showing the default beside a warning
      // about the configured daemon would quietly revert the operator's
      // choice the next time they pressed it.
      if (generation !== readGeneration.current) return;
      setConnection(current);
    } catch {
      // command not available yet
    }
    try {
      const [settings, status] = await Promise.all([
        invoke<DaemonSettings>("get_daemon_settings"),
        invoke<NodeStatus>("daemon_status"),
      ]);
      if (generation !== readGeneration.current) return;
      setDaemonSettings(settings);
      setDaemonStatus(status);
    } catch {
      // daemon commands not available yet
    }
  }, []);

  // Each read is asked for once the one before it has finished, never on
  // a fixed beat: a read slower than the beat would otherwise be numbered
  // out by the next one every time, and nothing would ever be shown. A
  // change of target still numbers out the read in flight.
  useEffect(() => {
    let closed = false;
    let timer: ReturnType<typeof setTimeout>;
    const read = async () => {
      await loadDaemonInfo();
      if (!closed) timer = setTimeout(read, SETTINGS_POLL_MS);
    };
    timer = setTimeout(read, 0);
    return () => {
      closed = true;
      clearTimeout(timer);
    };
  }, [loadDaemonInfo]);

  // The node the wallet starts lives as long as the wallet. The setting
  // stays where people look for it, and the backend answers in words that
  // a node which outlives the wallet is not built yet.
  async function handleKeepRunningAsk() {
    try {
      await invoke<DaemonSettings>("set_daemon_settings", {
        keepRunningOnExit: true,
      });
    } catch (e) {
      setKeepRunningAnswer(String(e));
    }
  }

  // The node reads its listener at its start: changing it starts the node
  // again, and the node's own words come back if it then refuses.
  async function handleInboundToggle(on: boolean) {
    setInboundBusy(true);
    setInboundError(null);
    try {
      await invoke<DaemonSettings>("set_daemon_settings", { clearnetInbound: on });
    } catch (e) {
      setInboundError(String(e));
    } finally {
      setInboundBusy(false);
      loadDaemonInfo();
    }
  }

  // The backend applied a change of network or of node address.
  function applied(now: DaemonConnection) {
    setConnection(now);
    refresh();
    loadDaemonInfo();
  }

  async function handleNetworkSwitch(net: string) {
    setSaving(true);
    setNetworkError(null);
    try {
      // No URL: the backend points at this computer's node, and relaunches
      // the wallet's own node on the network.
      applied(
        await invoke<DaemonConnection>("set_daemon_connection", { network: net }),
      );
    } catch (e) {
      // A wallet is open on another network: the switch waits for it.
      setNetworkError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Settings</h1>
      <WalletCare />

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
              disabled={saving || network === null}
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
        {networkError && <p className="text-xs text-amber-200">{networkError}</p>}
      </div>

      {/* Daemon */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <Server className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Daemon Connection
          </h2>
        </div>
        <NodeAddress connection={connection} onApplied={applied} />
        {health && (
          <p className="text-center text-[10px] text-emerald-400" title={BLOCK_NUMBER_TIP}>
            Connected — block {health.height.toLocaleString()}
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
                daemonStatus.state === "running"
                  ? "bg-emerald-400"
                  : daemonStatus.state === "starting" ||
                    daemonStatus.state === "stopping" ||
                    daemonStatus.state === "waiting"
                    ? "bg-amber-400"
                    : "bg-red-400"
              }`}
            />
            <span className="text-purple-300">
              {NODE_STATE_WORDS[daemonStatus.state]} ({daemonStatus.network})
            </span>
          </div>
        )}

        <TorSettings inForce={daemonSettings?.tor_fix ?? null} onChanged={loadDaemonInfo} />

        {daemonSettings && (
          <div className="space-y-1">
            <label className="flex items-center justify-between gap-3">
              <span className="text-xs text-purple-300">
                Accept incoming connections over clearnet
              </span>
              <button
                role="switch"
                aria-checked={daemonSettings.clearnet_inbound}
                aria-label="Accept incoming connections over clearnet"
                disabled={inboundBusy}
                onClick={() => handleInboundToggle(!daemonSettings.clearnet_inbound)}
                className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors disabled:opacity-50 ${
                  daemonSettings.clearnet_inbound ? "bg-gold-500" : "bg-purple-700"
                }`}
              >
                <span
                  className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
                    daemonSettings.clearnet_inbound ? "translate-x-4" : "translate-x-0"
                  }`}
                />
              </button>
            </label>
            <p className="text-xs text-purple-400">
              Off: your node connects out to other nodes and does not listen for
              them. On: other nodes can connect to yours over the open network,
              which shows them this computer’s address. Changing it restarts the
              node.
            </p>
            {inboundBusy && (
              <p className="text-xs text-purple-300">Starting the node again…</p>
            )}
            {inboundError && (
              <p role="alert" className="whitespace-pre-line text-xs text-red-300">
                {inboundError}
              </p>
            )}
          </div>
        )}

        {daemonSettings && (
          <div className="space-y-1">
            <label className="flex items-center justify-between gap-3">
              <span className="text-xs text-purple-300">
                Keep daemon running after wallet closes
              </span>
              <button
                role="switch"
                aria-checked={false}
                onClick={handleKeepRunningAsk}
                className="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent bg-purple-700 transition-colors"
              >
                <span className="pointer-events-none inline-block h-4 w-4 translate-x-0 transform rounded-full bg-white shadow transition-transform" />
              </button>
            </label>
            {keepRunningAnswer && (
              <p role="status" className="text-xs text-amber-200">
                {keepRunningAnswer}
              </p>
            )}
          </div>
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
        <button className="btn btn-danger w-full text-xs" onClick={() => void lockWallet()}>
          Close Wallet
        </button>
      </div>
    </div>
  );
}
