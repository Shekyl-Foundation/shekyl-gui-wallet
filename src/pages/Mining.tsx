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
import { formatSkl, formatHashRate } from "../lib/format";
import EmissionGauge from "../components/EmissionGauge";
import type { MiningStatus } from "../types/daemon";

export default function Mining() {
  const { health } = useDaemon();
  const [status, setStatus] = useState<MiningStatus | null>(null);
  const [threads, setThreads] = useState(2);
  const [background, setBackground] = useState(false);
  const [address, setAddress] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const maxThreads =
    typeof navigator !== "undefined" && navigator.hardwareConcurrency
      ? navigator.hardwareConcurrency
      : 8;

  const fetchStatus = useCallback(async () => {
    try {
      const ms = await invoke<MiningStatus>("get_mining_status");
      setStatus(ms);
      if (ms.address && !address) setAddress(ms.address);
      setError(null);
    } catch {
      setStatus(null);
    }
  }, [address]);

  useEffect(() => {
    fetchStatus();
    const id = setInterval(fetchStatus, 5_000);
    return () => clearInterval(id);
  }, [fetchStatus]);

  async function handleStart() {
    if (!address.trim()) {
      setError("Enter a miner address");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await invoke("start_mining_cmd", { address, threads, background });
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

  const hashRatePct = status?.active
    ? Math.min((status.speed / Math.max(status.difficulty / (status.block_target || 120), 1)) * 100, 100)
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
              Mining rewards are locked for 60 blocks (~2 hours) before
              becoming spendable. Mined outputs enter the normal mix ring,
              preserving transaction privacy.
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

        <div className="grid grid-cols-4 gap-4">
          <EmissionGauge
            value={hashRatePct}
            label="Hash Rate"
            display={status?.active ? formatHashRate(status.speed * (status.block_target || 120)) : "0 H/s"}
            size={70}
          />
          <div className="flex flex-col items-center justify-center gap-1">
            <Cpu className="h-5 w-5 text-gold-400" />
            <span className="text-sm font-bold text-white">
              {status?.threads_count ?? 0}
            </span>
            <span className="text-[10px] text-purple-300">Threads</span>
          </div>
          <div className="flex flex-col items-center justify-center gap-1">
            <Gauge className="h-5 w-5 text-gold-400" />
            <span className="text-sm font-bold text-white">
              {status?.active ? `${status.speed} H/s` : "—"}
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
              {status?.pow_algorithm || "RandomX"}
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
                {health.difficulty.toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-purple-300">Block Reward</p>
              <p className="font-mono font-bold text-white">
                {formatSkl(health.last_block_reward)} SKL
              </p>
            </div>
            <div>
              <p className="text-purple-300">Block Time</p>
              <p className="font-mono font-bold text-white">120s target</p>
            </div>
          </div>
        </div>
      )}

      {/* Controls */}
      <div className="card space-y-4">
        <h2 className="text-sm font-semibold text-purple-200">
          Mining Controls
        </h2>

        <div className="space-y-2">
          <label className="text-xs text-purple-300">Miner Address</label>
          <input
            type="text"
            className="input font-mono text-sm"
            placeholder="SKL1..."
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            disabled={status?.active}
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
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
          <div className="flex items-center gap-3">
            <label className="flex cursor-pointer items-center gap-2">
              <input
                type="checkbox"
                checked={background}
                onChange={(e) => setBackground(e.target.checked)}
                className="accent-gold-400"
                disabled={status?.active}
              />
              <span className="text-xs text-purple-300">
                Background mining
              </span>
            </label>
          </div>
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

        <p className="text-center text-[10px] text-purple-400">
          Requires an unrestricted daemon (do not use --restricted-rpc).
        </p>
      </div>
    </div>
  );
}
