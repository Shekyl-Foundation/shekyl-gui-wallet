import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Coins, Lock, TrendingUp } from "lucide-react";

interface StakingInfo {
  active: boolean;
  tier: number | null;
  staked_amount: number;
  unlock_height: number | null;
  rewards_pending: number;
  stake_ratio: number;
}

const TIERS = [
  { id: 0, label: "Tier 0", description: "Short lock period, lower rewards" },
  { id: 1, label: "Tier 1", description: "Medium lock period, moderate rewards" },
  { id: 2, label: "Tier 2", description: "Long lock period, highest rewards" },
];

function atomicToSkl(atomic: number): string {
  return (atomic / 1e12).toFixed(4);
}

export default function Staking() {
  const [info, setInfo] = useState<StakingInfo | null>(null);

  useEffect(() => {
    invoke<StakingInfo>("get_staking_info").then(setInfo).catch(() => {});
  }, []);

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Staking</h1>

      {/* Current status */}
      <div className="card">
        <div className="grid grid-cols-3 gap-4">
          <div className="text-center">
            <Lock className="mx-auto mb-2 h-5 w-5 text-gold-400" />
            <p className="text-xs text-purple-300">Staked</p>
            <p className="text-lg font-bold text-white">
              {info ? atomicToSkl(info.staked_amount) : "—"} SKL
            </p>
          </div>
          <div className="text-center">
            <TrendingUp className="mx-auto mb-2 h-5 w-5 text-gold-400" />
            <p className="text-xs text-purple-300">Pending Rewards</p>
            <p className="text-lg font-bold text-white">
              {info ? atomicToSkl(info.rewards_pending) : "—"} SKL
            </p>
          </div>
          <div className="text-center">
            <Coins className="mx-auto mb-2 h-5 w-5 text-gold-400" />
            <p className="text-xs text-purple-300">Stake Ratio</p>
            <p className="text-lg font-bold text-white">
              {info ? `${(info.stake_ratio * 100).toFixed(1)}%` : "—"}
            </p>
          </div>
        </div>
      </div>

      {/* Tier selection */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-purple-200">Select Tier</h2>
        {TIERS.map((tier) => (
          <button
            key={tier.id}
            className="card flex w-full items-center gap-4 text-left transition-colors hover:border-gold-500/50"
          >
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gold-500/10 text-gold-400 font-bold">
              {tier.id}
            </div>
            <div className="flex-1">
              <p className="text-sm font-semibold text-white">{tier.label}</p>
              <p className="text-xs text-purple-300">{tier.description}</p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
