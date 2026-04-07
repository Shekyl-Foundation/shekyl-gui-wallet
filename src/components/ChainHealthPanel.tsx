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
import { Activity, Flame, TrendingUp, Layers, Coins, Info, ShieldCheck } from "lucide-react";

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

  const progress = emissionProgress(health.already_generated_coins);
  const stakeRatioPct = (health.stake_ratio / 1_000_000) * 100;
  const emSharePct = (health.staker_emission_share_effective / 1_000_000) * 100;

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
          {health.emission_era && (
            <span className="rounded-full bg-gold-500/15 px-2 py-0.5 text-[10px] font-semibold text-gold-400">
              {health.emission_era}
            </span>
          )}
        </div>
        <div className="grid grid-cols-4 gap-3 text-center">
          <EmissionGauge
            value={stakeRatioPct}
            label="Stake Ratio"
            display={formatPercent(health.stake_ratio)}
          />
          <EmissionGauge
            value={health.release_multiplier / 1_000_000 * 50}
            max={100}
            label="Release Tempo"
            display={formatMultiplier(health.release_multiplier)}
          />
          <EmissionGauge
            value={(health.burn_pct / 1_000_000) * 100}
            label="Burn Rate"
            display={formatPercent(health.burn_pct)}
          />
          <EmissionGauge
            value={emSharePct}
            label="Emission Share"
            display={formatPercent(health.staker_emission_share_effective)}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Emission Era + Progress */}
      <div className="card">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Layers className="h-4 w-4 text-gold-400" />
            <h3 className="text-sm font-semibold text-purple-200">Emission</h3>
          </div>
          {health.emission_era && (
            <span className="rounded-full bg-gold-500/15 px-2.5 py-0.5 text-xs font-semibold text-gold-400">
              {health.emission_era} Era
            </span>
          )}
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
        <div className="grid grid-cols-2 gap-6 sm:grid-cols-4">
          <EmissionGauge
            value={stakeRatioPct}
            label="Stake Ratio"
            display={formatPercent(health.stake_ratio)}
          />
          <EmissionGauge
            value={health.release_multiplier / 1_000_000 * 50}
            max={100}
            label="Release Tempo"
            display={formatMultiplier(health.release_multiplier)}
          />
          <EmissionGauge
            value={(health.burn_pct / 1_000_000) * 100}
            label="Burn Rate"
            display={formatPercent(health.burn_pct)}
          />
          <EmissionGauge
            value={emSharePct}
            label="Staker Emission Share"
            display={formatPercent(health.staker_emission_share_effective)}
          />
        </div>
      </div>

      {/* Counters */}
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div className="card text-center">
          <Flame className="mx-auto mb-1 h-4 w-4 text-red-400" />
          <p className="text-[10px] text-purple-300">Total Burned</p>
          <p className="text-sm font-bold text-white">{formatSklCompact(health.total_burned)} SKL</p>
        </div>
        <div className="card text-center">
          <Coins className="mx-auto mb-1 h-4 w-4 text-gold-400" />
          <p className="text-[10px] text-purple-300">Staker Pool</p>
          <p className="text-sm font-bold text-white">{formatSklCompact(health.staker_pool_balance)} SKL</p>
        </div>
        <div className="card text-center">
          <TrendingUp className="mx-auto mb-1 h-4 w-4 text-emerald-400" />
          <p className="text-[10px] text-purple-300">Total Staked</p>
          <p className="text-sm font-bold text-white">{formatSklCompact(health.total_staked)} SKL</p>
        </div>
        <div className="card text-center">
          <ShieldCheck className="mx-auto mb-1 h-4 w-4 text-emerald-400" />
          <p className="text-[10px] text-purple-300">Anonymity Set</p>
          <p className="text-sm font-bold text-white">{(health.curve_tree_leaf_count ?? 0).toLocaleString()} outputs</p>
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
            <span className="font-mono text-white">{formatHashRate(health.difficulty)}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Last reward</span>
            <span className="font-mono text-white">{formatSkl(health.last_block_reward)} SKL</span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Tx pool</span>
            <span className="font-mono text-white">{health.tx_pool_size}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Release tempo</span>
            <span className="font-mono text-white">
              {formatMultiplier(health.release_multiplier)} — {tempoLabel(health.release_multiplier)}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-purple-300">Node version</span>
            <span className="font-mono text-white">{health.version}</span>
          </div>
          {(health.curve_tree_depth ?? 0) > 0 && (
            <>
              <div className="flex justify-between">
                <span className="text-purple-300">Curve tree depth</span>
                <span className="font-mono text-white">{health.curve_tree_depth}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-purple-300">Tree root</span>
                <span className="font-mono text-white">
                  {health.curve_tree_root?.slice(0, 8)}...
                </span>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
