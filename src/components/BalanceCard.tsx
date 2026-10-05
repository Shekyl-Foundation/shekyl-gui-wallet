import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../lib/errors";
import { atomicAmount, formatSkl } from "../lib/format";
import type { Balance } from "../types/daemon";

/** Shown in place of a staking figure the engine could not read (absent on the wire). */
const STAKING_UNAVAILABLE =
  "The wallet's staking state could not be read, so this figure is unavailable — it is not zero. Your spendable balance is unaffected.";

type BalanceRead = { kind: "loading" } | { kind: "ready"; balance: Balance } | { kind: "fault"; message: string };

/**
 * One figure's three render states: no figure (still loading, or the read
 * failed) is a dash, a present amount is formatted SKL, and an amount the wire
 * left absent is "Unavailable" — never a zero (rule 82: a non-value is not a
 * value). `precision` 9 keeps dust visible where dust is the whole point.
 */
function figure(atomic: string | undefined, read: BalanceRead, precision: 6 | 9 = 6): string {
  if (read.kind !== "ready") return "— SKL";
  if (atomic === undefined) return "Unavailable";
  return `${formatSkl(atomic, precision)} SKL`;
}

/**
 * The contract's `get_balance` at a glance. The headline is `liquid`; the
 * grid binds "Available" to `unlocked`, the spendable-now figure, so the two
 * stay honest if the engine ever splits them. A failed read is shown with a
 * retry, not swallowed into zeros.
 */
export default function BalanceCard() {
  const [read, setRead] = useState<BalanceRead>({ kind: "loading" });
  /** Monotonic generation so a late response cannot overwrite a retry. */
  const gen = useRef(0);

  const readBalance = useCallback(() => {
    const mine = ++gen.current;
    setRead({ kind: "loading" });
    invoke<Balance>("get_balance")
      .then((balance) => {
        if (mine === gen.current) setRead({ kind: "ready", balance });
      })
      .catch((err: unknown) => {
        if (mine === gen.current) setRead({ kind: "fault", message: describeError(err) });
      });
  }, []);

  useEffect(() => {
    readBalance();
    return () => {
      gen.current += 1;
    };
  }, [readBalance]);

  const balance = read.kind === "ready" ? read.balance : undefined;
  const stakingRead = balance?.staked !== undefined;
  const unspendable = balance && atomicAmount(balance.unspendable) > 0n ? balance.unspendable : undefined;

  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-3">
        <img src="/assets/shekyl_symbol.svg" alt="SKL" className="h-8 w-8" />
        <div>
          <p className="text-xs text-purple-300">Balance</p>
          <p className="text-2xl font-bold text-gold-400">{figure(balance?.liquid, read)}</p>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-4 border-t border-purple-700/50 pt-4">
        <div>
          <p className="text-xs text-purple-300" title="Spendable right now.">
            Available
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.unlocked, read)}</p>
        </div>
        <div>
          <p className="text-xs text-purple-300" title="Committed to a send that is awaiting confirmation.">
            Pending
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.pending, read)}</p>
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
          <p className="text-sm font-semibold text-white">{figure(balance?.staked, read)}</p>
        </div>
        <div>
          <p
            className="text-xs text-purple-300"
            title={stakingRead ? "Emission rewards received and still unspent." : STAKING_UNAVAILABLE}
          >
            Rewards
          </p>
          <p className="text-sm font-semibold text-white">{figure(balance?.claimable_rewards, read)}</p>
        </div>
      </div>

      {unspendable && (
        <p
          className="text-[11px] text-purple-300"
          title="Received on chain, but this wallet can never spend it. It is counted in no other figure."
          data-testid="unspendable"
        >
          Unspendable: {figure(unspendable, read, 9)}
        </p>
      )}

      {read.kind === "fault" && (
        <div
          role="alert"
          className="space-y-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2"
        >
          <p className="text-xs text-red-200">Balance could not be read: {read.message}</p>
          <button
            type="button"
            onClick={readBalance}
            className="text-xs font-medium text-purple-200 underline underline-offset-2 hover:text-white"
          >
            Try again
          </button>
        </div>
      )}
    </div>
  );
}
