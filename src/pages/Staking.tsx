import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Coins, Lock, TrendingUp, ShieldCheck, Info } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { formatSklCompact, formatPercent } from "../lib/format";
import EmissionGauge from "../components/EmissionGauge";
import StakeTierCard from "../components/StakeTierCard";
import type { TierYield } from "../types/daemon";

export default function Staking() {
  const { health } = useDaemon();
  const [tiers, setTiers] = useState<TierYield[]>([]);
  const [selectedTier, setSelectedTier] = useState<number | null>(null);

  useEffect(() => {
    invoke<TierYield[]>("get_tier_yields").then(setTiers).catch(() => {});
  }, [health]);

  const stakeRatioPct = health ? (health.stake_ratio / 1_000_000) * 100 : 0;
  const emSharePct = health
    ? (health.staker_emission_share_effective / 1_000_000) * 100
    : 0;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Staking</h1>

      {/* Privacy narrative */}
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
        <div className="flex items-start gap-3">
          <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
          <div>
            <p className="text-sm font-semibold text-emerald-300">
              Staking as Privacy Participation
            </p>
            <p className="mt-1 text-xs leading-relaxed text-emerald-200/80">
              Staked funds commingle in the accrual pool. Claims draw from
              pooled rewards, providing plausible deniability on the source of
              yield. Staking is both yield generation and privacy participation.
            </p>
          </div>
        </div>
      </div>

      {/* Network staking gauges */}
      {health && (
        <div className="card">
          <div className="mb-4 flex items-center gap-2">
            <Coins className="h-4 w-4 text-gold-400" />
            <h2 className="text-sm font-semibold text-purple-200">
              Network Staking
            </h2>
          </div>
          <div className="grid grid-cols-4 gap-4">
            <EmissionGauge
              value={stakeRatioPct}
              label="Stake Ratio"
              display={formatPercent(health.stake_ratio)}
            />
            <EmissionGauge
              value={emSharePct}
              label="Emission Share"
              display={formatPercent(health.staker_emission_share_effective)}
            />
            <div className="flex flex-col items-center justify-center gap-1">
              <Lock className="h-5 w-5 text-gold-400" />
              <span className="text-sm font-bold text-white">
                {formatSklCompact(health.total_staked)}
              </span>
              <span className="text-[10px] text-purple-300">Total Staked</span>
            </div>
            <div className="flex flex-col items-center justify-center gap-1">
              <TrendingUp className="h-5 w-5 text-gold-400" />
              <span className="text-sm font-bold text-white">
                {formatSklCompact(health.staker_pool_balance)}
              </span>
              <span className="text-[10px] text-purple-300">Reward Pool</span>
            </div>
          </div>
        </div>
      )}

      {/* Tier selection */}
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <h2 className="text-sm font-semibold text-purple-200">
            Select Staking Tier
          </h2>
          <span
            className="cursor-help text-purple-400"
            title="Longer lock periods earn higher yield multipliers. APY estimates are based on current network conditions."
          >
            <Info className="h-3.5 w-3.5" />
          </span>
        </div>
        {tiers.map((tier) => (
          <StakeTierCard
            key={tier.tier}
            tier={tier}
            selected={selectedTier === tier.tier}
            onSelect={() => setSelectedTier(tier.tier)}
          />
        ))}
        {tiers.length === 0 && (
          <p className="text-center text-xs text-purple-300">
            Connect to a daemon to see tier data
          </p>
        )}
      </div>

      {/* Stake action */}
      <div className="card space-y-4">
        <h2 className="text-sm font-semibold text-purple-200">Stake SKL</h2>
        <input
          type="number"
          className="input"
          placeholder="Amount to stake"
          min="0"
          step="0.000001"
        />
        <button
          className="btn btn-primary w-full"
          disabled={selectedTier === null}
        >
          <Lock className="h-4 w-4" />
          {selectedTier !== null
            ? `Stake at Tier ${selectedTier}`
            : "Select a tier to stake"}
        </button>
        <p className="text-center text-[10px] text-purple-400">
          Wallet connection required. Staking transactions will be available
          once the wallet2 FFI bridge is integrated.
        </p>
      </div>
    </div>
  );
}
