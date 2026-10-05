import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseSkl, formatSkl } from "../../lib/format";

interface Built {
  pending_tx_id: string;
  fee: string;
  content_gen: number;
}

interface Receipt {
  tx_hash: string;
  verdict: string;
  confirmed_height: number | null;
}

interface CollectReceipt {
  kind: "swept" | "nothing_left";
  tx_hash: string | null;
  swept: string | null;
  remainder: string | null;
  another_pool_remains: boolean | null;
}

function messageOf(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  return e instanceof Error ? e.message : String(e);
}

/**
 * Fund, return, release, and collect. The buttons use those words.
 * `unstake` is the contract command behind Release.
 */
export default function StakeActions() {
  const [amount, setAmount] = useState("");
  const [returnAmount, setReturnAmount] = useState("");
  const [built, setBuilt] = useState<Built | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirmRelease, setConfirmRelease] = useState(false);

  const clear = () => {
    setError(null);
    setNote(null);
  };

  const fund = async () => {
    clear();
    let atomic: bigint;
    try {
      atomic = parseSkl(amount);
    } catch (e) {
      setError(messageOf(e));
      return;
    }
    setBusy(true);
    try {
      const pending = await invoke<Built>("stake_in", { amount: atomic.toString() });
      setBuilt(pending);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const confirmFund = async () => {
    if (!built) return;
    setBusy(true);
    clear();
    try {
      const result = await invoke<Receipt>("submit_pending_tx", {
        pendingTxId: built.pending_tx_id,
        seenGen: built.content_gen,
      });
      setNote(`Funds sent (${result.verdict}). They join your stake after the network confirms.`);
      setBuilt(null);
      setAmount("");
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const cancelFund = async () => {
    if (!built) return;
    setBusy(true);
    try {
      await invoke("discard_pending_tx", { pendingTxId: built.pending_tx_id });
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBuilt(null);
      setBusy(false);
    }
  };

  const doReturn = async () => {
    clear();
    let atomic: bigint;
    try {
      atomic = parseSkl(returnAmount);
    } catch (e) {
      setError(messageOf(e));
      return;
    }
    if (atomic === 0n) {
      setError("The amount must be greater than zero.");
      return;
    }
    setBusy(true);
    try {
      const result = await invoke<Receipt>("drain", { amount: atomic.toString() });
      setNote(
        result.verdict === "ALREADY_IN_CHAIN"
          ? `An earlier return is already confirmed (${result.tx_hash}).`
          : `Return sent (${result.tx_hash}). It arrives in your balance after confirmation.`,
      );
      setReturnAmount("");
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const doRelease = async () => {
    clear();
    setBusy(true);
    try {
      const result = await invoke<Receipt>("unstake");
      setNote(
        result.verdict === "ALREADY_IN_CHAIN"
          ? `Release is already confirmed. Collect the funds into this wallet.`
          : `Release posted (${result.tx_hash}). After it confirms, collect the funds.`,
      );
      setConfirmRelease(false);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const doCollect = async () => {
    clear();
    setBusy(true);
    try {
      const result = await invoke<CollectReceipt>("collect_unstaked");
      if (result.kind === "nothing_left") {
        setNote("Nothing left to collect.");
      } else if (result.another_pool_remains) {
        setNote("This pass was sent. Released funds from another stake still remain. Collect again after this confirms.");
      } else if (result.remainder && result.remainder !== "0") {
        setNote("This pass was sent. Some funds are not spendable yet. Collect again after this confirms.");
      } else {
        setNote("Collection sent. Once it confirms, nothing further remains.");
      }
    } catch (e) {
      setError(messageOf(e));
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
