import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Pickaxe,
  Play,
  Square,
  Cpu,
  Gauge,
  ShieldCheck,
  Info,
} from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { coinbaseLockText, formatSkl, formatHashRate } from "../lib/format";
import { useChainConstants } from "../hooks/useChainConstants";
import EmissionGauge from "../components/EmissionGauge";
import type { MiningStatus } from "../types/daemon";

export default function Mining() {
  const { health } = useDaemon();
  const constants = useChainConstants();
  const lockWindow = coinbaseLockText(
    constants?.coinbase_lock_window ?? null,
    health?.block_spacing ?? null,
  );
  const [status, setStatus] = useState<MiningStatus | null>(null);
  const [threads, setThreads] = useState(2);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Why the node's miner could not be read: the node's own sentence, such
  // as a node that does not let this wallet see or change mining.
  const [statusError, setStatusError] = useState<string | null>(null);

  const maxThreads =
    typeof navigator !== "undefined" && navigator.hardwareConcurrency
      ? navigator.hardwareConcurrency
      : 8;

  const fetchStatus = useCallback(async () => {
    try {
      const ms = await invoke<MiningStatus>("get_mining_status");
      setStatus(ms);
      setStatusError(null);
    } catch (e) {
      setStatus(null);
      setStatusError(String(e));
    }
  }, []);

  useEffect(() => {
    fetchStatus();
    const id = setInterval(fetchStatus, 5_000);
    return () => clearInterval(id);
  }, [fetchStatus]);

  async function handleStart() {
    setLoading(true);
    setError(null);
    try {
      await invoke("start_mining_cmd", { threads });
      await fetchStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleStop() {
    setLoading(true);
    setError(null);
    try {
      await invoke("stop_mining_cmd");
      await fetchStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  // This miner's share of the network's hash rate, estimated from the
  // posted job's difficulty over the block spacing.
  const networkRate =
    status?.difficulty && status.block_spacing > 0
      ? Number(status.difficulty) / status.block_spacing
      : null;
  const hashRatePct =
    status?.active && status.hash_rate !== null && networkRate
      ? Math.min((status.hash_rate / Math.max(networkRate, 1)) * 100, 100)
      : 0;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Mining</h1>

      {/* Privacy note */}
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
        <div className="flex items-start gap-3">
          <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
          <div>
            <p className="text-sm font-semibold text-emerald-300">
              Mining and Privacy
            </p>
            <p className="mt-1 text-xs leading-relaxed text-emerald-200/80">
              {lockWindow === null
                ? "Mining rewards wait a fixed number of blocks before they can be spent."
                : `Mining rewards can be spent after ${lockWindow}.`}{" "}
              Shekyl has no rings: every output, a mined one included, is in
              the full-chain set each spend proves membership in, so spending
              a reward looks like any other spend.
            </p>
          </div>
        </div>
      </div>

      {/* Status */}
      <div className="card">
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Pickaxe className="h-4 w-4 text-gold-400" />
            <h2 className="text-sm font-semibold text-purple-200">
              Mining Status
            </h2>
          </div>
          <span
            className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[10px] font-semibold ${
              status?.active
                ? "bg-emerald-500/15 text-emerald-400"
                : "bg-purple-700/50 text-purple-300"
            }`}
          >
            <span
              className={`h-1.5 w-1.5 rounded-full ${
                status?.active ? "bg-emerald-400 animate-pulse" : "bg-purple-400"
              }`}
            />
            {status?.active ? "Mining" : "Idle"}
          </span>
        </div>

        {statusError && (
          <p className="mb-4 rounded-lg bg-purple-700/30 p-2 text-xs text-purple-200">
            {statusError}
          </p>
        )}
        {status?.cause && (
          <p className="mb-4 rounded-lg bg-purple-700/30 p-2 text-xs text-purple-200">
            {status.cause}
          </p>
        )}

        <div className="grid grid-cols-4 gap-4">
          <EmissionGauge
            value={hashRatePct}
            label="Hash Rate"
            display={
              status === null
                ? "—"
                : !status.active
                  ? "0 H/s"
                  : status.hash_rate === null
                    ? "—"
                    : formatHashRate(status.hash_rate)
            }
            size={70}
          />
          <div className="flex flex-col items-center justify-center gap-1">
            <Cpu className="h-5 w-5 text-gold-400" />
            <span className="text-sm font-bold text-white">
              {status?.threads ?? "—"}
            </span>
            <span className="text-[10px] text-purple-300">Threads</span>
          </div>
          <div className="flex flex-col items-center justify-center gap-1">
            <Gauge className="h-5 w-5 text-gold-400" />
            <span className="text-sm font-bold text-white">
              {status?.active && status.hash_rate !== null
                ? formatHashRate(status.hash_rate)
                : "—"}
            </span>
            <span className="text-[10px] text-purple-300">
              Speed
              <span
                className="ml-1 cursor-help text-purple-400"
                title="Hashes per second your CPU computes using the RandomX algorithm"
              >
                <Info className="inline h-2.5 w-2.5" />
              </span>
            </span>
          </div>
          <div className="flex flex-col items-center justify-center gap-1">
            <Pickaxe className="h-5 w-5 text-gold-400" />
            <span className="text-sm font-bold text-white">
              RandomX
            </span>
            <span className="text-[10px] text-purple-300">Algorithm</span>
          </div>
        </div>
      </div>

      {/* Network context */}
      {health && (
        <div className="card">
          <h3 className="mb-3 text-xs font-semibold uppercase tracking-wider text-purple-300">
            Network Mining
          </h3>
          <div className="grid grid-cols-3 gap-4 text-center text-xs">
            <div>
              <p className="text-purple-300">Difficulty</p>
              <p className="font-mono font-bold text-white">
                {BigInt(health.difficulty).toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-purple-300">Block Reward</p>
              <p className="font-mono font-bold text-white">
                {health.last_block ? `${formatSkl(health.last_block.reward)} SKL` : "—"}
              </p>
            </div>
            <div>
              <p className="text-purple-300">Block Time</p>
              <p className="font-mono font-bold text-white">{health.block_spacing}s target</p>
            </div>
          </div>
        </div>
      )}

      {/* Controls */}
      <div className="card space-y-4">
        <h2 className="text-sm font-semibold text-purple-200">
          Mining Controls
        </h2>

        <div className="space-y-1">
          <p className="text-xs text-purple-300">Pays to</p>
          <p className="break-all font-mono text-sm text-white">
            {status === null
              ? "—"
              : (status.address ?? "No mining address is set on this node.")}
          </p>
          <p className="text-[10px] text-purple-400">
            The node mines to the address it was started with.
          </p>
        </div>

        <div className="space-y-2">
          <label className="text-xs text-purple-300">
            Threads (1–{maxThreads})
          </label>
          <input
            type="range"
            min={1}
            max={maxThreads}
            value={threads}
            onChange={(e) => setThreads(Number(e.target.value))}
            className="w-full accent-gold-400"
            disabled={status?.active}
          />
          <p className="text-center text-xs font-semibold text-white">
            {threads}
          </p>
        </div>

        {error && (
          <p className="rounded-lg bg-red-500/10 p-2 text-xs text-red-400">
            {error}
          </p>
        )}

        {status?.active ? (
          <button
            onClick={handleStop}
            disabled={loading}
            className="btn btn-danger flex w-full items-center justify-center gap-2"
          >
            <Square className="h-4 w-4" />
            {loading ? "Stopping..." : "Stop Mining"}
          </button>
        ) : (
          <button
            onClick={handleStart}
            disabled={loading}
            className="btn btn-primary flex w-full items-center justify-center gap-2"
          >
            <Play className="h-4 w-4" />
            {loading ? "Starting..." : "Start Mining"}
          </button>
        )}
      </div>
    </div>
  );
}
