import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Globe, Server, Shield, ShieldCheck, Settings2 } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import SecurityPanel from "../components/SecurityPanel";
import WalletCare from "../components/WalletCare";
import { BLOCK_NUMBER_TIP } from "../lib/format";
import type { NodeState, NodeStatus } from "../types/node";

/** How often Settings reads the node's state while it is open. */
const SETTINGS_POLL_MS = 2000;

interface DaemonSettings {
  data_dir: string | null;
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

/**
 * What the backend says about the daemon URL now in effect. `warnings` is
 * empty for a daemon on this machine — the default and the supported
 * configuration — and never contains an assurance: the only thing this
 * wallet can establish about a daemon address is what it costs, never that
 * it is safe.
 */
interface DaemonConnection {
  /** The node address a person typed; `null` is this computer's node. */
  url: string | null;
  warnings: string[];
}

const NETWORKS = ["mainnet", "testnet", "stagenet"] as const;


export default function Settings() {
  const { health, refresh } = useDaemon();
  const [network, setNetwork] = useState<string>(health?.network ?? "mainnet");
  // The address a person typed, as the backend keeps it; empty for this
  // computer's node, which has no address to show (it is read over its
  // owner's leg).
  const [daemonUrl, setDaemonUrl] = useState("");
  const [typedNode, setTypedNode] = useState(false);
  // Whether the field has been edited since it was loaded: the reads do
  // not overwrite what the person is typing. Not state: never drawn.
  const urlEditedRef = useRef(false);
  // Each read of the node's state is numbered; an answer older than the
  // last one shown is dropped, so a slow read never paints over a newer one.
  const readGeneration = useRef(0);
  const [networkError, setNetworkError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [daemonWarnings, setDaemonWarnings] = useState<string[]>([]);
  const [daemonSettings, setDaemonSettings] = useState<DaemonSettings | null>(null);
  const [daemonStatus, setDaemonStatus] = useState<NodeStatus | null>(null);
  const [keepRunningAnswer, setKeepRunningAnswer] = useState<string | null>(null);
  const [inboundBusy, setInboundBusy] = useState(false);
  const [inboundError, setInboundError] = useState<string | null>(null);
  // The node's Tor state, in core's words; or why it cannot be read.
  const [torRead, setTorRead] = useState<string | null>(null);

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
      setTypedNode(current.url !== null);
      // The field is the person's while they edit it.
      setDaemonUrl((shown) => (urlEditedRef.current ? shown : (current.url ?? "")));
      setDaemonWarnings(current.warnings);
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
    try {
      const tor = await invoke<{ shown: boolean; text: string } | null>(
        "get_tor_status",
      );
      if (generation !== readGeneration.current) return;
      setTorRead(tor ? tor.text : null);
    } catch (e) {
      if (generation !== readGeneration.current) return;
      // The node is not answering: its own notice says why.
      setTorRead(String(e));
    }
  }, []);

  useEffect(() => {
    const first = setTimeout(loadDaemonInfo, 0);
    const timer = setInterval(loadDaemonInfo, SETTINGS_POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
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

  async function handleNetworkSwitch(net: string) {
    setSaving(true);
    setNetworkError(null);
    try {
      // No URL: the backend points at this computer's node, and relaunches
      // the wallet's own node on the network.
      const applied = await invoke<DaemonConnection>("set_daemon_connection", {
        network: net,
      });
      setNetwork(net);
      urlEditedRef.current = false;
      setDaemonUrl(applied.url ?? "");
      setTypedNode(applied.url !== null);
      setDaemonWarnings(applied.warnings);
      refresh();
      loadDaemonInfo();
    } catch (e) {
      // A wallet is open on another network: the switch waits for it.
      setNetworkError(String(e));
    } finally {
      setSaving(false);
    }
  }

  // An address is sent when the field holds one; an empty field is this
  // computer's node. A typed node stays typed until "Use this computer's
  // node".
  async function handleSaveUrl() {
    setSaving(true);
    try {
      const url = daemonUrl.trim() === "" ? undefined : daemonUrl.trim();
      const applied = await invoke<DaemonConnection>("set_daemon_connection", {
        network,
        ...(url === undefined ? {} : { url }),
      });
      urlEditedRef.current = false;
      setTypedNode(applied.url !== null);
      setDaemonUrl(applied.url ?? "");
      setDaemonWarnings(applied.warnings);
      refresh();
      loadDaemonInfo();
    } catch {
      // ignore
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
        <div className="space-y-2">
          <label className="text-xs text-purple-300">Node address</label>
          <input
            type="text"
            className="input font-mono text-sm"
            placeholder="Empty: this computer’s node"
            value={daemonUrl}
            onChange={(e) => {
              urlEditedRef.current = true;
              setDaemonUrl(e.target.value);
            }}
          />
          <p className="text-[11px] text-purple-400">
            {typedNode
              ? "A node address you typed: the wallet asks it over the network."
              : "This computer’s node, the one the wallet starts."}
          </p>
        </div>
        <button
          onClick={handleSaveUrl}
          disabled={saving}
          className="btn btn-secondary w-full text-xs"
        >
          {saving ? "Connecting..." : "Save & Reconnect"}
        </button>
        {typedNode && (
          <button
            onClick={() => handleNetworkSwitch(network)}
            disabled={saving}
            className="btn btn-ghost w-full text-xs"
          >
            Use this computer’s node
          </button>
        )}
        {daemonWarnings.map((warning) => (
          <p
            key={warning}
            className="rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[11px] leading-relaxed text-amber-200"
            role="status"
          >
            {warning}
          </p>
        ))}
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

        {torRead && (
          <div className="space-y-1">
            <h3 className="text-xs font-semibold text-purple-200">Tor</h3>
            <p data-testid="tor-read" className="whitespace-pre-line text-xs text-purple-300">
              {torRead}
            </p>
          </div>
        )}

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
        <button className="btn btn-danger w-full text-xs">
          Close Wallet
        </button>
      </div>
    </div>
  );
}
