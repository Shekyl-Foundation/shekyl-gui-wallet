import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { atomicAmount, formatSkl } from "../lib/format";
import type { Balance } from "../types/daemon";

/** Shown in place of a staking figure the engine could not read (absent on the wire). */
const STAKING_UNAVAILABLE =
  "The wallet's staking state could not be read, so this figure is unavailable — it is not zero. Your spendable balance is unaffected.";

/**
 * One figure's three render states: no data yet (or a failed read) is a dash,
 * a present amount is formatted SKL, and an amount the wire left absent is
 * "Unavailable" — never a zero (rule 82: a non-value is not a value).
 */
function figure(atomic: string | undefined, loaded: boolean): string {
  if (!loaded) return "— SKL";
  if (atomic === undefined) return "Unavailable";
  return `${formatSkl(atomic)} SKL`;
}

/** The contract's `get_balance` at a glance. */
export default function BalanceCard() {
  const [balance, setBalance] = useState<Balance | null>(null);

  useEffect(() => {
    invoke<Balance>("get_balance").then(setBalance).catch(() => {});
  }, []);

  const loaded = balance !== null;
  const stakingRead = loaded && balance.staked !== undefined;
  const unspendable = loaded && atomicAmount(balance.unspendable) > 0n ? balance.unspendable : undefined;

  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-3">
        <img src="/assets/shekyl_symbol.svg" alt="SKL" className="h-8 w-8" />
        <div>
          <p className="text-xs text-purple-300">Available</p>
          <p className="text-2xl font-bold text-gold-400">{figure(balance?.liquid, loaded)}</p>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-4 border-t border-purple-700/50 pt-4">
        <div>
          <p className="text-xs text-purple-300" title="Committed to a send that is awaiting confirmation.">
            Pending
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.pending, loaded)}</p>
        </div>
        <div>
          <p
            className="text-xs text-purple-300"
            title={
              stakingRead
                ? "Bond principal under your confirmed and in-flight bonds. The Staking page shows the legs."
                : STAKING_UNAVAILABLE
            }
          >
            Staked
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.staked, loaded)}</p>
        </div>
        <div>
          <p
            className="text-xs text-purple-300"
            title={stakingRead ? "Emission rewards received and still unspent." : STAKING_UNAVAILABLE}
          >
            Rewards
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.claimable_rewards, loaded)}</p>
        </div>
      </div>

      {unspendable && (
        <p
          className="text-[11px] text-purple-300"
          title="Received on chain, but this wallet can never spend it. It is counted in no other figure."
          data-testid="unspendable"
        >
          Unspendable: {figure(unspendable, loaded)}
        </p>
      )}
    </div>
  );
}
