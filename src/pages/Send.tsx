import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send as SendIcon, AlertCircle, ShieldCheck, Loader2, Info } from "lucide-react";
import type {
  BuiltPendingTx,
  FeePriorityTier,
  FeeTierQuote,
  SubmitResult,
} from "../types/send";
import { isSendError } from "../types/send";

/**
 * One built transaction per user intent, and the fee the user confirms is the
 * fee of the transaction that ships.
 *
 * compose  — typing. The fee shown is the daemon's tier quote for the
 *            canonical shape (weight × rate), fetched once per page. Nothing
 *            is built while typing.
 * review   — `build_pending_tx` ran once; the exact fee, amount and recipient
 *            are shown. Confirm submits that reservation with its
 *            `content_gen`. Cancel, or leaving the page, discards it.
 * sent     — the network's verdict.
 *
 * If the engine reports the content changed (the realized fee or change moved
 * on re-anchor), the reservation is discarded and rebuilt, and the user is
 * returned to review with numbers they can read. It is never resubmitted
 * without them.
 */
type Phase = "compose" | "building" | "review" | "submitting" | "sent";

const TIERS: { tier: FeePriorityTier; label: string; hint: string }[] = [
  { tier: "ECONOMY", label: "Economy", hint: "cheapest; a few blocks" },
  { tier: "STANDARD", label: "Standard", hint: "balanced" },
  { tier: "PRIORITY", label: "Priority", hint: "next block" },
];

function formatAtomicSkl(atomic: number): string {
  return (atomic / 1e9).toFixed(4);
}

function sklToAtomic(value: string): number {
  const [whole = "0", frac = ""] = value.split(".");
  const padded = (frac + "000000000").slice(0, 9);
  const atomic = BigInt(whole || "0") * BigInt(1_000_000_000) + BigInt(padded);
  return Number(atomic);
}

function quoteFor(quote: FeeTierQuote | null, tier: FeePriorityTier): number | null {
  if (!quote) return null;
  switch (tier) {
    case "ECONOMY":
      return quote.economy_fee;
    case "STANDARD":
      return quote.standard_fee;
    case "PRIORITY":
      return quote.priority_fee;
  }
}

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
  const [amount, setAmount] = useState("");
  const [priority, setPriority] = useState<FeePriorityTier>("STANDARD");
  const [quote, setQuote] = useState<FeeTierQuote | null>(null);
  const [built, setBuilt] = useState<BuiltPendingTx | null>(null);
  const [sent, setSent] = useState<SubmitResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // The reservation the page currently owns, for discard on unmount. Cleared
  // the moment ownership ends (submitted, discarded, or retained by the engine).
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

  const discardOwned = useCallback(async () => {
    const id = owned.current;
    if (!id) return;
    owned.current = null;
    try {
      await invoke("discard_pending_tx", { pendingTxId: id });
    } catch {
      // The reservation may already be gone; nothing the page can do here.
    }
  }, []);

  // Leaving the page discards a reservation the page still owns.
  useEffect(() => {
    return () => {
      void discardOwned();
    };
  }, [discardOwned]);

  const build = useCallback(async (): Promise<BuiltPendingTx | null> => {
    setPhase("building");
    try {
      const b = await invoke<BuiltPendingTx>("build_pending_tx", {
        address,
        amount: sklToAtomic(amount),
        priority,
      });
      owned.current = b.pending_tx_id;
      setBuilt(b);
      setPhase("review");
      return b;
    } catch (e) {
      setError(isSendError(e) ? e.message : String(e));
      setPhase("compose");
      return null;
    }
  }, [address, amount, priority]);

  async function handleReview(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setNotice(null);
    await build();
  }

  async function handleCancel() {
    setNotice(null);
    setError(null);
    await discardOwned();
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
        // The realized fee or change moved while the user was reviewing. The
        // engine exposes no view of the re-anchored reservation, so rather than
        // ask for consent to a fee nobody can see, release it and build again.
        await discardOwned();
        setBuilt(null);
        setNotice(
          "The chain moved while you were reviewing and the transaction's fee or " +
            "change would have differed. It has been rebuilt — please check the " +
            "figures again before confirming.",
        );
        await build();
        return;
      }
      if (isSendError(e) && e.reservation_retained) {
        // Ambiguous or still pending on the network: the engine keeps the
        // reservation so a retry cannot double-spend. The page must not
        // discard it.
        owned.current = null;
        setError(
          `${e.message} Refresh your balance and check Transactions before trying again.`,
        );
        setPhase("compose");
        setBuilt(null);
        return;
      }
      // Anything else: the engine has released the funds; start over.
      owned.current = null;
      setError(isSendError(e) ? e.message : String(e));
      setBuilt(null);
      setPhase("compose");
    }
  }

  function handleSendAnother() {
    setSent(null);
    setBuilt(null);
    setAddress("");
    setAmount("");
    setError(null);
    setNotice(null);
    setPhase("compose");
  }

  const busy = phase === "building" || phase === "submitting";
  const quoted = quoteFor(quote, priority);
  const amountAtomic = amount ? sklToAtomic(amount) : 0;

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
              disabled={busy || phase === "review"}
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-purple-200">Amount (SKL)</label>
            <input
              type="number"
              className="input"
              placeholder="0.0000"
              step="0.0001"
              min="0"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              required
              disabled={busy || phase === "review"}
            />
          </div>

          <fieldset className="space-y-2" disabled={busy || phase === "review"}>
            <legend className="text-sm font-medium text-purple-200">Fee priority</legend>
            <div className="grid grid-cols-3 gap-2">
              {TIERS.map(({ tier, label, hint }) => {
                const q = quoteFor(quote, tier);
                return (
                  <label
                    key={tier}
                    className={`cursor-pointer rounded-lg border px-3 py-2 text-xs ${
                      priority === tier
                        ? "border-gold-500 bg-gold-500/10 text-white"
                        : "border-purple-600/30 bg-purple-800/30 text-purple-200"
                    }`}
                  >
                    <input
                      type="radio"
                      name="priority"
                      value={tier}
                      className="sr-only"
                      checked={priority === tier}
                      onChange={() => setPriority(tier)}
                    />
                    <span className="block font-medium">{label}</span>
                    <span className="block text-[10px] opacity-80">{hint}</span>
                    <span className="block font-mono text-gold-400">
                      {q === null ? "estimate unavailable" : `≈ ${formatAtomicSkl(q)} SKL`}
                    </span>
                  </label>
                );
              })}
            </div>
            {quoted !== null && phase === "compose" && (
              <p className="text-[11px] text-purple-300">
                Estimates for a typical transaction. The exact fee is shown before you confirm.
              </p>
            )}
          </fieldset>

          {notice && (
            <div className="flex items-start gap-2 rounded-lg border border-orange-500/40 bg-orange-900/20 p-3 text-xs text-orange-200" role="status">
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
            <div className="space-y-2 rounded-lg border border-gold-500/40 bg-purple-800/40 p-3 text-sm" data-testid="review">
              <div className="flex justify-between text-purple-200">
                <span>To</span>
                <span className="max-w-[60%] break-all text-right font-mono text-xs text-white">{address}</span>
              </div>
              <div className="flex justify-between text-purple-200">
                <span>Amount</span>
                <span className="font-mono text-white">{formatAtomicSkl(amountAtomic)} SKL</span>
              </div>
              <div className="flex justify-between text-purple-200">
                <span>Fee</span>
                <span className="font-mono text-gold-400" data-testid="exact-fee">
                  {formatAtomicSkl(built.fee)} SKL
                </span>
              </div>
              <div className="flex justify-between border-t border-purple-600/40 pt-2 font-medium text-white">
                <span>Total</span>
                <span className="font-mono">{formatAtomicSkl(amountAtomic + built.fee)} SKL</span>
              </div>
              <p className="flex items-center gap-1 text-[10px] text-emerald-300">
                <ShieldCheck className="h-2.5 w-2.5" />
                Full-chain membership proof with post-quantum protection
              </p>
            </div>
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
