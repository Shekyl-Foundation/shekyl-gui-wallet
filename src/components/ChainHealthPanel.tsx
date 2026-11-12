import { useDaemon } from "../context/useDaemon";
import {
  formatSkl,
  formatSklCompact,
  formatPercent,
  formatMultiplier,
  formatHashRate,
  emissionProgress,
} from "../lib/format";
import EmissionGauge from "./EmissionGauge";
import { Activity, Flame, Layers, Info, ShieldCheck } from "lucide-react";

interface Props {
  compact?: boolean;
}

function tempoLabel(multiplier: number): string {
  const m = multiplier / 1_000_000;
  if (m >= 1.5) return "High activity";
  if (m >= 1.1) return "Moderately active";
  if (m >= 0.9) return "Steady";
  return "Low activity";
}

export default function ChainHealthPanel({ compact = false }: Props) {
  const { health, loading, error } = useDaemon();

  if (loading) {
    return (
      <div className="card animate-pulse">
        <div className="h-24 rounded bg-purple-700/30" />
      </div>
    );
  }

  if (error || !health) {
    return (
      <div className="card text-center text-sm text-purple-300">
        {error ? `Daemon unavailable: ${error}` : "No daemon data"}
      </div>
    );
  }

  const progress = emissionProgress(health.emission);
  const emSharePct = (health.stake_share / 1_000_000) * 100;
  // The network's hash rate, estimated from difficulty over block spacing.
  const hashRate =
    health.block_spacing > 0 ? Number(health.difficulty) / health.block_spacing : null;

  if (compact) {
    return (
      <div className="card">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-purple-300">
            Chain Health
            <span
              className="cursor-help text-purple-400"
              title="Live network metrics from the connected daemon — difficulty, emission, staking, and burn statistics"
            >
              <Info className="inline h-3 w-3" />
            </span>
          </h3>
        </div>
        <div className="grid grid-cols-3 gap-3 text-center">
          <EmissionGauge
            value={health.emission_multiplier / 1_000_000 * 50}
            max={100}
            label="Release Tempo"
            display={formatMultiplier(health.emission_multiplier)}
          />
          <EmissionGauge
            value={(health.burn / 1_000_000) * 100}
            label="Burn Rate"
            display={formatPercent(health.burn)}
          />
          <EmissionGauge
            value={emSharePct}
            label="Emission Share"
            display={formatPercent(health.stake_share)}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Emission progress */}
      <div className="card">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Layers className="h-4 w-4 text-gold-400" />
            <h3 className="text-sm font-semibold text-purple-200">Emission</h3>
          </div>
        </div>
        <div className="space-y-2">
          <div className="flex justify-between text-xs text-purple-300">
            <span>Supply progress</span>
            <span>{progress.toFixed(2)}%</span>
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-purple-700/50">
            <div
              className="h-full rounded-full bg-gold-400 transition-all duration-700"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      </div>

      {/* Gauges */}
      <div className="card">
        <div className="grid grid-cols-3 gap-6">
          <EmissionGauge
            value={health.emission_multiplier / 1_000_000 * 50}
            max={100}
            label="Release Tempo"
            display={formatMultiplier(health.emission_multiplier)}
          />
          <EmissionGauge
            value={(health.burn / 1_000_000) * 100}
            label="Burn Rate"
            display={formatPercent(health.burn)}
          />
          <EmissionGauge
            value={emSharePct}
            label="Staker Emission Share"
            display={formatPercent(health.stake_share)}
          />
        </div>
      </div>

      {/* Counters */}
      <div className="grid grid-cols-2 gap-4">
        <div className="card text-center">
          <Flame className="mx-auto mb-1 h-4 w-4 text-red-400" />
          <p className="text-[10px] text-purple-300">Total Burned</p>
          <p className="text-sm font-bold text-white">{formatSklCompact(health.burn_total)} SKL</p>
        </div>
        <div className="card text-center">
          <ShieldCheck className="mx-auto mb-1 h-4 w-4 text-emerald-400" />
          <p className="text-[10px] text-purple-300">Anonymity Set</p>
          <p className="text-sm font-bold text-white">
            {health.curve_tree ? `${health.curve_tree.leaf_count.toLocaleString()} outputs` : "—"}
          </p>
        </div>
      </div>

      {/* Network stats */}
      <div className="card">
        <div className="mb-3 flex items-center gap-2">
          <Activity className="h-4 w-4 text-gold-400" />
          <h3 className="text-sm font-semibold text-purple-200">Network</h3>
        </div>
        <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
          <div className="flex justify-between">
            <span className="text-purple-300">Block height</span>
            <span className="font-mono text-white">{health.height.toLocaleString()}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Hash rate</span>
            <span className="font-mono text-white">
              {hashRate === null ? "—" : formatHashRate(hashRate)}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Last reward</span>
            <span className="font-mono text-white">
              {health.last_block ? `${formatSkl(health.last_block.reward)} SKL` : "—"}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Tx pool</span>
            <span className="font-mono text-white">{health.txpool_count ?? "—"}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Release tempo</span>
            <span className="font-mono text-white">
              {formatMultiplier(health.emission_multiplier)} — {tempoLabel(health.emission_multiplier)}
            </span>
          </div>
          {health.curve_tree && (
            <>
              <div className="flex justify-between">
                <span className="text-purple-300">Curve tree layers</span>
                <span className="font-mono text-white">{health.curve_tree.layers}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-purple-300">Tree root</span>
                <span className="font-mono text-white">
                  {health.curve_tree.root.slice(0, 8)}...
                </span>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
