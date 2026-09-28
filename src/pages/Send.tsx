import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send as SendIcon, AlertCircle, ShieldCheck, Loader2, Info } from "lucide-react";
import { FeeTierPicker, ReviewCard } from "../components/send";
import { atomicAmount, formatSkl, parseSkl, SKL_AMOUNT_PATTERN, SKL_DECIMALS } from "../lib/format";
import type {
  BuiltPendingTx,
  FeePriorityTier,
  FeeTierQuote,
  SubmitResult,
} from "../types/send";
import { isSendError, sendErrorMessage } from "../types/send";
import { PAYMENT_URI_SCHEME, type ParsedPaymentUri } from "../types/receiving";

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

function isPaymentLink(value: string): boolean {
  return value.trim().toLowerCase().startsWith(PAYMENT_URI_SCHEME);
}

/** What the payer should check. The label rides the link; nothing from it is sent. */
function paymentLinkNotice(link: ParsedPaymentUri): string {
  const label = link.label ? ` — "${link.label}"` : "";
  const request = link.rid ? ` (request ${link.rid})` : "";
  return `Filled from a payment link${label}${request}. Check the address and amount before you review.`;
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
  const [amountText, setAmountText] = useState("");
  const [priority, setPriority] = useState<FeePriorityTier>("STANDARD");
  const [quote, setQuote] = useState<FeeTierQuote | null>(null);
  const [built, setBuilt] = useState<BuiltPendingTx | null>(null);
  const [sent, setSent] = useState<SubmitResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  /** The payment link the recipient field was filled from, if any (shown, never trusted). */
  const [linkNotice, setLinkNotice] = useState<string | null>(null);
  /**
   * A `shekyl:` value is being parsed. Review stays disabled until it
   * settles, so a build cannot capture the pre-link address while the
   * card later shows the parsed one.
   */
  const [readingLink, setReadingLink] = useState(false);
  /** Mirrors `readingLink` for the submit handler, which can run before the next paint. */
  const readingLinkRef = useRef(false);
  /** Latest recipient edit. A parse that is no longer this generation is dropped. */
  const parseGeneration = useRef(0);
  /** Latest phase, so a parse that resolves after Review has started cannot rewrite the fields. */
  const phaseRef = useRef<Phase>("compose");
  /**
   * The address and amount a build will use. Updated in the same turn as
   * the field, including when a parse replaces them, so Review cannot
   * capture a stale render's values.
   */
  const addressRef = useRef("");
  const amountRef = useRef("");
  /** A discard is in flight: the review buttons are held so nothing can act on a reservation being released. */
  const [releasing, setReleasing] = useState(false);
  const owned = useRef<string | null>(null);

  function enterPhase(next: Phase) {
    phaseRef.current = next;
    setPhase(next);
  }

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
    setReleasing(true);
    try {
      await invoke("discard_pending_tx", { pendingTxId: id });
      owned.current = null;
      return true;
    } catch (e) {
      setError(`${sendErrorMessage(e)} ${RELEASE_FAILED_ADVICE}`);
      return false;
    } finally {
      setReleasing(false);
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
    const recipient = addressRef.current;
    const amountInput = amountRef.current;
    let amount: bigint;
    try {
      amount = parseSkl(amountInput);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
      return false;
    }
    enterPhase("building");
    try {
      const b = await invoke<BuiltPendingTx>("build_pending_tx", {
        address: recipient,
        amount: amount.toString(),
        priority,
      });
      owned.current = b.pending_tx_id;
      setBuilt(b);
      enterPhase("review");
      return true;
    } catch (e) {
      setError(sendErrorMessage(e));
      enterPhase("compose");
      return false;
    }
  }, [priority]);

  function setReading(next: boolean) {
    readingLinkRef.current = next;
    setReadingLink(next);
  }

  /**
   * A `shekyl:` payment link pasted into the recipient field is parsed by
   * Rust (`parse_uri`) and fills the address and amount. The typed value is
   * committed immediately. The result is applied only if it is still the
   * latest edit and the page is still composing — a build already in flight
   * keeps the address and amount it captured. The label is shown as text
   * from the counterparty and is not sent.
   */
  async function handleAddressChange(value: string) {
    const generation = ++parseGeneration.current;
    addressRef.current = value;
    setAddress(value);
    if (!isPaymentLink(value)) {
      setReading(false);
      setLinkNotice(null); // edited by hand: no link describes the field
      return;
    }
    setReading(true);
    try {
      const link = await invoke<ParsedPaymentUri>("parse_uri", { uri: value.trim() });
      if (generation !== parseGeneration.current || phaseRef.current !== "compose") return;
      // Refs move first, then the reading gate drops, so a submit in this
      // same turn builds the parsed address rather than the raw link.
      addressRef.current = link.address;
      setAddress(link.address);
      if (link.amount !== undefined) {
        const formatted = formatSkl(link.amount, SKL_DECIMALS);
        amountRef.current = formatted;
        setAmountText(formatted);
      }
      setLinkNotice(paymentLinkNotice(link));
      setError(null);
    } catch (e) {
      if (generation !== parseGeneration.current || phaseRef.current !== "compose") return;
      setLinkNotice(null);
      setError(sendErrorMessage(e));
    } finally {
      if (generation === parseGeneration.current) setReading(false);
    }
  }

  async function handleReview(e: React.FormEvent) {
    e.preventDefault();
    // Enter in the field submits even when the button is disabled. A link
    // still being read, or a build already started, must not build again.
    if (readingLinkRef.current || phaseRef.current !== "compose") return;
    setError(null);
    setNotice(null);
    await build();
  }

  async function handleCancel() {
    setNotice(null);
    setError(null);
    if (!(await discardOwned())) return; // still ours; stay in review, error shown
    setBuilt(null);
    enterPhase("compose");
  }

  async function handleConfirm() {
    if (!built) return;
    setError(null);
    setNotice(null);
    enterPhase("submitting");
    try {
      const r = await invoke<SubmitResult>("submit_pending_tx", {
        pendingTxId: built.pending_tx_id,
        seenGen: built.content_gen,
      });
      owned.current = null;
      setSent(r);
      enterPhase("sent");
    } catch (e) {
      if (isSendError(e) && e.code === "CONTENT_GEN_MISMATCH") {
        // The realized fee or change moved. The engine exposes no view of the
        // re-anchored reservation, so release it and build again rather than
        // ask for consent to figures nobody can see. If the release fails the
        // old reservation is still ours: return to review and say so — never
        // stack a second reservation on top of a live one. The page reads as
        // building throughout, so nothing can act on the stale reservation
        // while it is being released.
        enterPhase("building");
        if (!(await discardOwned())) {
          enterPhase("review");
          return;
        }
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
        enterPhase("compose");
        return;
      }
      // Anything else: the engine has released the funds; start over.
      owned.current = null;
      setError(sendErrorMessage(e));
      setBuilt(null);
      enterPhase("compose");
    }
  }

  function handleSendAnother() {
    parseGeneration.current += 1;
    addressRef.current = "";
    amountRef.current = "";
    setSent(null);
    setBuilt(null);
    setAddress("");
    setAmountText("");
    setLinkNotice(null);
    setReading(false);
    setError(null);
    setNotice(null);
    enterPhase("compose");
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
              onChange={(e) => void handleAddressChange(e.target.value)}
              required
              disabled={locked}
            />
            {readingLink && (
              <p className="text-[11px] text-purple-300" role="status">
                Reading the payment link…
              </p>
            )}
            {linkNotice && (
              <p className="text-[11px] text-purple-300" data-testid="link-notice">
                {linkNotice}
              </p>
            )}
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
              onChange={(e) => {
                amountRef.current = e.target.value;
                setAmountText(e.target.value);
              }}
              required
              disabled={locked || readingLink}
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
              <button type="button" className="btn btn-ghost flex-1" onClick={handleCancel} disabled={releasing}>
                Cancel
              </button>
              <button type="button" className="btn btn-primary flex-1" onClick={handleConfirm} disabled={releasing}>
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
            <button type="submit" disabled={busy || readingLink} className="btn btn-primary w-full">
              <SendIcon className="h-4 w-4" />
              {readingLink ? "Reading link…" : phase === "building" ? "Building..." : "Review"}
            </button>
          )}
        </form>
      )}
    </div>
  );
}
