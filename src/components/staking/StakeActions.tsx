import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../../lib/errors";
import { formatSkl, parseSkl } from "../../lib/format";
import type { BuiltPendingTx, SubmitResult } from "../../types/send";
import {
  CONTENT_CHANGED_NOTICE,
  isSendError,
  RELEASE_FAILED_ADVICE,
  RETAINED_ADVICE,
  sendErrorMessage,
} from "../../types/send";

interface SealedReceipt {
  tx_hash: string;
  verdict: "BROADCAST" | "ALREADY_IN_CHAIN";
  confirmed_height?: number;
}

type CollectReceipt =
  | {
      status: "SWEPT";
      tx_hash: string;
      swept: string;
      remainder: string;
      another_pool_remains: boolean;
    }
  | { status: "NOTHING_LEFT" };

function fundNote(result: SubmitResult): string {
  switch (result.verdict) {
    case "ACCEPTED":
      return "Funds sent. They join your stake after the network confirms.";
    case "ALREADY_IN_POOL":
      return "These funds were already sent and are waiting to confirm.";
    case "ALREADY_IN_CHAIN":
      return "These funds are already confirmed. They join your stake after this wallet refreshes.";
  }
}

function collectNote(result: CollectReceipt): string {
  if (result.status === "NOTHING_LEFT") return "Nothing left to collect.";
  const immature = result.remainder !== "0";
  const another = result.another_pool_remains;
  if (immature && another) {
    return "This pass was sent. Some funds are not spendable yet, and released funds from another stake still remain. Collect again after this confirms.";
  }
  if (another) {
    return "This pass was sent. Released funds from another stake still remain. Collect again after this confirms.";
  }
  if (immature) {
    return "This pass was sent. Some funds are not spendable yet. Collect again after this confirms.";
  }
  return "Collection sent. Once it confirms, nothing further remains.";
}

/**
 * Fund, return, release, and collect. The buttons use those words.
 * `unstake` is the contract command behind Release.
 *
 * Fund owns one send reservation, the same way the Send page does. It is
 * released on a successful discard, a successful submit, or when the engine
 * says it has kept the reservation. A failed cancel stays here so the funds
 * are not forgotten. Leaving the page discards as best it can.
 */
export default function StakeActions() {
  const [amount, setAmount] = useState("");
  const [returnAmount, setReturnAmount] = useState("");
  const [built, setBuilt] = useState<BuiltPendingTx | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirmRelease, setConfirmRelease] = useState(false);
  /** Reservation this panel holds. Null once submit, discard, or the engine takes it. */
  const owned = useRef<string | null>(null);
  /** Atomic amount of the reservation under review, so a rebuild uses the same one. */
  const reviewedAmount = useRef<string | null>(null);

  const clear = () => {
    setError(null);
    setNote(null);
  };

  const discardOwned = useCallback(async (): Promise<boolean> => {
    const id = owned.current;
    if (!id) return true;
    try {
      await invoke("discard_pending_tx", { pendingTxId: id });
      owned.current = null;
      return true;
    } catch (err) {
      setError(`${sendErrorMessage(err)} ${RELEASE_FAILED_ADVICE}`);
      return false;
    }
  }, []);

  useEffect(() => {
    return () => {
      const id = owned.current;
      if (id) void invoke("discard_pending_tx", { pendingTxId: id }).catch(() => {});
    };
  }, []);

  const buildStake = async (atomic: string): Promise<boolean> => {
    try {
      const pending = await invoke<BuiltPendingTx>("stake_in", { amount: atomic });
      owned.current = pending.pending_tx_id;
      reviewedAmount.current = atomic;
      setBuilt(pending);
      return true;
    } catch (err) {
      setError(describeError(err));
      return false;
    }
  };

  const fund = async () => {
    if (owned.current) return;
    clear();
    let atomic: bigint;
    try {
      atomic = parseSkl(amount);
    } catch (err) {
      setError(describeError(err));
      return;
    }
    setBusy(true);
    try {
      await buildStake(atomic.toString());
    } finally {
      setBusy(false);
    }
  };

  const confirmFund = async () => {
    if (!built) return;
    setBusy(true);
    clear();
    try {
      const result = await invoke<SubmitResult>("submit_pending_tx", {
        pendingTxId: built.pending_tx_id,
        seenGen: built.content_gen,
      });
      owned.current = null;
      reviewedAmount.current = null;
      setNote(fundNote(result));
      setBuilt(null);
      setAmount("");
    } catch (err) {
      if (isSendError(err) && err.code === "CONTENT_GEN_MISMATCH") {
        if (!(await discardOwned())) return;
        setBuilt(null);
        setNote(CONTENT_CHANGED_NOTICE);
        const atomic = reviewedAmount.current;
        if (atomic) await buildStake(atomic);
        return;
      }
      if (isSendError(err) && err.reservation_retained) {
        owned.current = null;
        reviewedAmount.current = null;
        setError(`${err.message} ${RETAINED_ADVICE}`);
        setBuilt(null);
        return;
      }
      owned.current = null;
      reviewedAmount.current = null;
      setError(describeError(err));
      setBuilt(null);
    } finally {
      setBusy(false);
    }
  };

  const cancelFund = async () => {
    setBusy(true);
    setError(null);
    const released = await discardOwned();
    setBusy(false);
    if (!released) return;
    reviewedAmount.current = null;
    setBuilt(null);
  };

  const doReturn = async () => {
    clear();
    let atomic: bigint;
    try {
      atomic = parseSkl(returnAmount);
    } catch (err) {
      setError(describeError(err));
      return;
    }
    if (atomic === 0n) {
      setError("The amount must be greater than zero.");
      return;
    }
    setBusy(true);
    try {
      const result = await invoke<SealedReceipt>("drain", { amount: atomic.toString() });
      setNote(
        result.verdict === "ALREADY_IN_CHAIN"
          ? `An earlier return is already confirmed (${result.tx_hash}).`
          : `Return sent (${result.tx_hash}). It arrives in your balance after confirmation.`,
      );
      setReturnAmount("");
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const doRelease = async () => {
    clear();
    setBusy(true);
    try {
      const result = await invoke<SealedReceipt>("unstake");
      setNote(
        result.verdict === "ALREADY_IN_CHAIN"
          ? "Release is already confirmed. Collect the funds into this wallet."
          : `Release posted (${result.tx_hash}). After it confirms, collect the funds.`,
      );
      setConfirmRelease(false);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const doCollect = async () => {
    clear();
    setBusy(true);
    try {
      const result = await invoke<CollectReceipt>("collect_unstaked");
      setNote(collectNote(result));
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-4">
      <h2 className="text-sm font-semibold text-purple-200">Move funds</h2>
      <p className="text-xs text-purple-300">
        Fund adds money to the stake. Return brings staking funds back to this
        wallet. Release closes the bond permanently. Collect moves the released
        collateral into this wallet after Release confirms.
      </p>

      <div className="space-y-2">
        <p className="text-xs font-semibold text-purple-200">Fund</p>
        <p className="text-xs text-purple-300">
          This is an ordinary send from this wallet. A small extra amount is
          added automatically for privacy and stays yours inside the stake. The
          exact extra is chosen when you confirm and is not shown beforehand.
        </p>
        {!built && (
          <div className="flex gap-2">
            <input
              className="input"
              placeholder="Amount (SKL)"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              disabled={busy}
            />
            <button type="button" className="btn btn-primary" disabled={busy || amount.length === 0} onClick={() => void fund()}>
              Review
            </button>
          </div>
        )}
        {built && (
          <div className="space-y-2 text-xs text-purple-100">
            <p>Fee: {formatSkl(built.fee)} SKL</p>
            <div className="flex gap-2">
              <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void confirmFund()}>
                Fund stake
              </button>
              <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => void cancelFund()}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="space-y-2">
        <p className="text-xs font-semibold text-purple-200">Return</p>
        <div className="flex gap-2">
          <input
            className="input"
            placeholder="Amount (SKL)"
            value={returnAmount}
            onChange={(e) => setReturnAmount(e.target.value)}
            disabled={busy}
          />
          <button type="button" className="btn btn-ghost" disabled={busy || returnAmount.length === 0} onClick={() => void doReturn()}>
            Return
          </button>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-xs font-semibold text-purple-200">Release</p>
        {!confirmRelease ? (
          <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirmRelease(true)}>
            Release stake
          </button>
        ) : (
          <div className="space-y-2 text-xs text-amber-100">
            <p>
              Release closes this bond for good. Staking again later means a new
              bond. The funds come back to the staking balance first; Collect
              moves them into this wallet.
            </p>
            <div className="flex gap-2">
              <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void doRelease()}>
                Release permanently
              </button>
              <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirmRelease(false)}>
                Keep stake
              </button>
            </div>
          </div>
        )}
      </div>

      <div>
        <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => void doCollect()}>
          Collect
        </button>
      </div>

      {error && <p className="text-xs text-red-300">{error}</p>}
      {note && <p className="text-xs text-emerald-200">{note}</p>}
    </div>
  );
}
