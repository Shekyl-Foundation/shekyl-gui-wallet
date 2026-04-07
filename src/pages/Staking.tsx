import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Coins, Lock, TrendingUp, ShieldCheck, Info, Gift } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { formatSkl, formatSklCompact, formatPercent } from "../lib/format";
import EmissionGauge from "../components/EmissionGauge";
import StakeTierCard from "../components/StakeTierCard";
import type { TierYield, WalletStakingInfo, StakedOutput } from "../types/daemon";

export default function Staking() {
  const { health } = useDaemon();
  const [tiers, setTiers] = useState<TierYield[]>([]);
  const [selectedTier, setSelectedTier] = useState<number | null>(null);
  const [stakeAmount, setStakeAmount] = useState("");
  const [stakingInfo, setStakingInfo] = useState<WalletStakingInfo | null>(null);
  const [staking, setStaking] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const [stakeError, setStakeError] = useState<string | null>(null);

  const fetchStakingInfo = useCallback(() => {
    invoke<WalletStakingInfo>("get_staking_info")
      .then(setStakingInfo)
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<TierYield[]>("get_tier_yields").then(setTiers).catch(() => {});
    fetchStakingInfo();
  }, [health, fetchStakingInfo]);

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

      {/* Your stakes */}
      {stakingInfo && stakingInfo.staked_outputs.length > 0 && (
        <div className="card space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-purple-200">Your Stakes</h2>
            <span className="text-xs text-purple-300">
              Total: {formatSkl(stakingInfo.total_staked)} SKL
            </span>
          </div>
          <div className="space-y-2">
            {stakingInfo.staked_outputs.map((so: StakedOutput, idx: number) => (
              <div key={idx} className="flex items-center justify-between rounded-lg border border-purple-600/30 bg-purple-800/30 px-3 py-2 text-xs">
                <div>
                  <span className="font-semibold text-white">{formatSkl(so.amount)} SKL</span>
                  <span className="ml-2 text-purple-300">Tier {so.tier}</span>
                  <span className="ml-2 text-purple-400">
                    {so.claimable ? "Unlocked" : `Locked until block ${so.unlock_height.toLocaleString()}`}
                  </span>
                </div>
                {so.claimable && (
                  <button
                    className="btn btn-primary px-3 py-1 text-xs"
                    disabled={claiming}
                    onClick={async () => {
                      setClaiming(true);
                      setStakeError(null);
                      try {
                        await invoke("claim_rewards");
                        fetchStakingInfo();
                      } catch (e) {
                        setStakeError(String(e));
                      } finally {
                        setClaiming(false);
                      }
                    }}
                  >
                    <Gift className="h-3 w-3" />
                    {claiming ? "Claiming..." : "Claim"}
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Stake action */}
      <div className="card space-y-4">
        <h2 className="text-sm font-semibold text-purple-200">Stake SKL</h2>
        <input
          type="number"
          className="input"
          placeholder="Amount to stake"
          min="0"
          step="0.000001"
          value={stakeAmount}
          onChange={(e) => setStakeAmount(e.target.value)}
          disabled={staking}
        />
        {stakeError && (
          <p className="text-xs text-red-300">{stakeError}</p>
        )}
        <button
          className="btn btn-primary w-full"
          disabled={selectedTier === null || staking || !stakeAmount}
          onClick={async () => {
            if (selectedTier === null) return;
            setStaking(true);
            setStakeError(null);
            try {
              const [whole = "0", frac = ""] = stakeAmount.split(".");
              const padded = (frac + "000000000").slice(0, 9);
              const atomic = BigInt(whole) * BigInt(1_000_000_000) + BigInt(padded);
              await invoke("stake", {
                tier: selectedTier,
                amount: Number(atomic),
              });
              setStakeAmount("");
              fetchStakingInfo();
            } catch (e) {
              setStakeError(String(e));
            } finally {
              setStaking(false);
            }
          }}
        >
          <Lock className="h-4 w-4" />
          {staking
            ? "Staking..."
            : selectedTier !== null
              ? `Stake at Tier ${selectedTier}`
              : "Select a tier to stake"}
        </button>
      </div>
    </div>
  );
}
