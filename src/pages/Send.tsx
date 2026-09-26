import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send as SendIcon, AlertCircle, ShieldCheck, Loader2, Info } from "lucide-react";
import { FeeTierPicker, ReviewCard } from "../components/send";
import { atomicAmount, parseSkl, SKL_AMOUNT_PATTERN } from "../lib/format";
import type {
  BuiltPendingTx,
  FeePriorityTier,
  FeeTierQuote,
  SubmitResult,
} from "../types/send";
import { isSendError, sendErrorMessage } from "../types/send";

/**
 * One built transaction per user intent, and the fee the user confirms is the
 * fee of the transaction that ships.
 *
 *   compose → building → review → submitting → sent
 *
 * compose   Typing. The tier picker shows the daemon's quote for a typical
 *           transaction, fetched once per page. Nothing is built.
 * review    `build_pending_tx` ran once; the exact fee, amount and recipient
 *           are shown. Confirm submits that reservation with its
 *           `content_gen`. Cancel, or leaving the page, discards it.
 * sent      The network's verdict.
 *
 * Ownership: `owned` is the reservation this page holds. It is released only
 * when a submit succeeds, when a discard succeeds, or when the engine reports
 * it has kept the reservation itself. A failed discard keeps ownership and is
 * shown, so funds are never left locked behind a reservation the page has
 * forgotten.
 *
 * If the engine reports the content changed (the realized fee or change
 * moved on re-anchor), the reservation is discarded and rebuilt and the user
 * is returned to review with numbers they can read. It is never resubmitted
 * without them.
 */
type Phase = "compose" | "building" | "review" | "submitting" | "sent";

const CONTENT_CHANGED_NOTICE =
  "The chain moved while you were reviewing and the transaction's fee or change " +
  "would have differed. It has been rebuilt — please check the figures again before confirming.";
const RETAINED_ADVICE = "Refresh your balance and check Transactions before trying again.";
const RELEASE_FAILED_ADVICE =
  "The reservation could not be released; your funds stay reserved until it is. Try Cancel again.";

function verdictLine(r: SubmitResult): string {
  switch (r.verdict) {
    case "ACCEPTED":
      return "Transaction submitted.";
    case "ALREADY_IN_POOL":
      return "Transaction already in the network's mempool.";
    case "ALREADY_IN_CHAIN":
      return r.confirmed_height !== null
        ? `Transaction already confirmed on chain (reported height ${r.confirmed_height}).`
        : "Transaction already confirmed on chain.";
  }
}

export default function Send() {
  const [phase, setPhase] = useState<Phase>("compose");
  const [address, setAddress] = useState("");
  const [amountText, setAmountText] = useState("");
  const [priority, setPriority] = useState<FeePriorityTier>("STANDARD");
  const [quote, setQuote] = useState<FeeTierQuote | null>(null);
  const [built, setBuilt] = useState<BuiltPendingTx | null>(null);
  const [sent, setSent] = useState<SubmitResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const owned = useRef<string | null>(null);

  useEffect(() => {
    let live = true;
    invoke<FeeTierQuote>("get_default_fee_priority")
      .then((q) => live && setQuote(q))
      .catch(() => live && setQuote(null));
    return () => {
      live = false;
    };
  }, []);

  /**
   * Release the owned reservation. Ownership ends only on success; a failure
   * keeps it and reports why, so the page never forgets a live reservation.
   */
  const discardOwned = useCallback(async (): Promise<boolean> => {
    const id = owned.current;
    if (!id) return true;
    try {
      await invoke("discard_pending_tx", { pendingTxId: id });
      owned.current = null;
      return true;
    } catch (e) {
      setError(`${sendErrorMessage(e)} ${RELEASE_FAILED_ADVICE}`);
      return false;
    }
  }, []);

  // Leaving the page: best effort — there is no page left to show a failure.
  useEffect(() => {
    return () => {
      const id = owned.current;
      if (id) void invoke("discard_pending_tx", { pendingTxId: id }).catch(() => {});
    };
  }, []);

  const build = useCallback(async (): Promise<boolean> => {
    let amount: bigint;
    try {
      amount = parseSkl(amountText);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
      return false;
    }
    setPhase("building");
    try {
      const b = await invoke<BuiltPendingTx>("build_pending_tx", {
        address,
        amount: amount.toString(),
        priority,
      });
      owned.current = b.pending_tx_id;
      setBuilt(b);
      setPhase("review");
      return true;
    } catch (e) {
      setError(sendErrorMessage(e));
      setPhase("compose");
      return false;
    }
  }, [address, amountText, priority]);

  async function handleReview(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setNotice(null);
    await build();
  }

  async function handleCancel() {
    setNotice(null);
    setError(null);
    if (!(await discardOwned())) return; // still ours; stay in review, error shown
    setBuilt(null);
    setPhase("compose");
  }

  async function handleConfirm() {
    if (!built) return;
    setError(null);
    setNotice(null);
    setPhase("submitting");
    try {
      const r = await invoke<SubmitResult>("submit_pending_tx", {
        pendingTxId: built.pending_tx_id,
        seenGen: built.content_gen,
      });
      owned.current = null;
      setSent(r);
      setPhase("sent");
    } catch (e) {
      if (isSendError(e) && e.code === "CONTENT_GEN_MISMATCH") {
        // The realized fee or change moved. The engine exposes no view of the
        // re-anchored reservation, so release it and build again rather than
        // ask for consent to figures nobody can see. If the release fails the
        // old reservation is still ours: stay in review and say so — never
        // stack a second reservation on top of a live one.
        setPhase("review");
        if (!(await discardOwned())) return;
        setBuilt(null);
        setNotice(CONTENT_CHANGED_NOTICE);
        await build();
        return;
      }
      if (isSendError(e) && e.reservation_retained) {
        // Ambiguous or still pending on the network: the engine keeps the
        // reservation so a retry cannot double-spend. It is no longer ours.
        owned.current = null;
        setError(`${e.message} ${RETAINED_ADVICE}`);
        setBuilt(null);
        setPhase("compose");
        return;
      }
      // Anything else: the engine has released the funds; start over.
      owned.current = null;
      setError(sendErrorMessage(e));
      setBuilt(null);
      setPhase("compose");
    }
  }

  function handleSendAnother() {
    setSent(null);
    setBuilt(null);
    setAddress("");
    setAmountText("");
    setError(null);
    setNotice(null);
    setPhase("compose");
  }

  const busy = phase === "building" || phase === "submitting";
  const locked = busy || phase === "review";

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Send SKL</h1>

      {phase === "sent" && sent ? (
        <div className="card space-y-4">
          <div className="flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm text-emerald-300">
            <ShieldCheck className="h-4 w-4" />
            {verdictLine(sent)}
          </div>
          <p className="break-all font-mono text-xs text-purple-200">{sent.tx_hash}</p>
          <button type="button" className="btn btn-primary w-full" onClick={handleSendAnother}>
            Send another
          </button>
        </div>
      ) : (
        <form onSubmit={handleReview} className="card space-y-5">
          <div className="space-y-2">
            <label className="text-sm font-medium text-purple-200">Recipient Address</label>
            <input
              type="text"
              className="input font-mono text-sm"
              placeholder="shekyl1..."
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              required
              disabled={locked}
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-purple-200">Amount (SKL)</label>
            <input
              type="text"
              inputMode="decimal"
              className="input"
              placeholder="0.0000"
              pattern={SKL_AMOUNT_PATTERN}
              value={amountText}
              onChange={(e) => setAmountText(e.target.value)}
              required
              disabled={locked}
            />
          </div>

          <FeeTierPicker quote={quote} value={priority} onChange={setPriority} disabled={locked} />

          {notice && (
            <div
              className="flex items-start gap-2 rounded-lg border border-orange-500/40 bg-orange-900/20 p-3 text-xs text-orange-200"
              role="status"
            >
              <Info className="mt-0.5 h-4 w-4 shrink-0" />
              {notice}
            </div>
          )}

          {error && (
            <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300" role="alert">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
              {error}
            </div>
          )}

          {phase === "building" && (
            <div className="flex items-center gap-2 rounded-lg border border-purple-600/50 bg-purple-800/40 p-3 text-xs text-purple-200">
              <Loader2 className="h-3.5 w-3.5 animate-spin text-gold-400" />
              Building the transaction and its membership proof — this can take a while on slower devices.
            </div>
          )}

          {(phase === "review" || phase === "submitting") && built && (
            <ReviewCard address={address} amount={parseSkl(amountText)} fee={atomicAmount(built.fee)} />
          )}

          {phase === "review" && (
            <div className="flex gap-2">
              <button type="button" className="btn btn-ghost flex-1" onClick={handleCancel}>
                Cancel
              </button>
              <button type="button" className="btn btn-primary flex-1" onClick={handleConfirm}>
                <SendIcon className="h-4 w-4" />
                Confirm and send
              </button>
            </div>
          )}

          {phase === "submitting" && (
            <button type="button" className="btn btn-primary w-full" disabled>
              <Loader2 className="h-4 w-4 animate-spin" />
              Submitting...
            </button>
          )}

          {(phase === "compose" || phase === "building") && (
            <button type="submit" disabled={busy} className="btn btn-primary w-full">
              <SendIcon className="h-4 w-4" />
              {phase === "building" ? "Building..." : "Review"}
            </button>
          )}
        </form>
      )}
    </div>
  );
}
