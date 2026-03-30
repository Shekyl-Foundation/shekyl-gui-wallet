import { Lock } from "lucide-react";
import { formatDuration } from "../lib/format";
import type { TierYield } from "../types/daemon";

interface Props {
  tier: TierYield;
  selected?: boolean;
  onSelect?: () => void;
}

const TIER_NAMES = ["Short", "Medium", "Long"];

export default function StakeTierCard({ tier, selected, onSelect }: Props) {
  return (
    <button
      onClick={onSelect}
      className={`card flex w-full items-center gap-4 text-left transition-colors ${
        selected
          ? "border-gold-500 bg-gold-500/10"
          : "hover:border-gold-500/50"
      }`}
    >
      <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-gold-500/10">
        <Lock className={`h-5 w-5 ${selected ? "text-gold-400" : "text-purple-300"}`} />
      </div>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <p className="text-sm font-semibold text-white">
            Tier {tier.tier} — {TIER_NAMES[tier.tier] ?? "Custom"}
          </p>
          <span className="rounded bg-purple-700/50 px-1.5 py-0.5 text-[10px] font-semibold text-purple-300">
            {tier.yield_multiplier}x
          </span>
        </div>
        <p className="text-xs text-purple-300">
          Lock: {formatDuration(tier.lock_duration_hours)} ({tier.lock_blocks.toLocaleString()} blocks)
        </p>
      </div>
      <div className="text-right">
        <p className="text-sm font-bold text-gold-400">
          {tier.estimated_apy.toFixed(1)}%
        </p>
        <p className="text-[10px] text-purple-300">Est. APY</p>
      </div>
    </button>
  );
}
