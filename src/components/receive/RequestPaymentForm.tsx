import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2 } from "lucide-react";
import { parseSkl, SKL_AMOUNT_PATTERN } from "../../lib/format";
import { describeError } from "../../lib/errors";
import type { CreatedPaymentRequest } from "../../types/receiving";

/** Expiry choices, as durations from now; `null` is "no expiry". */
const EXPIRY_CHOICES: readonly { label: string; seconds: number | null }[] = [
  { label: "No expiry", seconds: null },
  { label: "1 hour", seconds: 60 * 60 },
  { label: "24 hours", seconds: 24 * 60 * 60 },
  { label: "7 days", seconds: 7 * 24 * 60 * 60 },
];
const MAX_LABEL_CHARS = 256;

interface RequestPaymentFormProps {
  /** Called with the created request; the page shows its URI and QR. */
  onCreated: (created: CreatedPaymentRequest) => void;
}

/**
 * The contract's `create_payment_request`: amount, a label for your own
 * bookkeeping, optional expiry. The request is stored in the wallet and the
 * returned `shekyl:` link carries its reference, so a payment made from the
 * link is matched to it when it arrives.
 */
export default function RequestPaymentForm({ onCreated }: RequestPaymentFormProps) {
  const [amountText, setAmountText] = useState("");
  const [label, setLabel] = useState("");
  const [expiryIndex, setExpiryIndex] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSubmit = !busy && amountText.trim().length > 0 && label.length <= MAX_LABEL_CHARS;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    let amount: bigint;
    try {
      amount = parseSkl(amountText);
    } catch (err) {
      setError(describeError(err));
      return;
    }
    const seconds = EXPIRY_CHOICES[expiryIndex].seconds;
    const expiry = seconds === null ? undefined : Math.floor(Date.now() / 1000) + seconds;
    setBusy(true);
    try {
      const created = await invoke<CreatedPaymentRequest>("create_payment_request", {
        label: label.trim(),
        amount: amount.toString(),
        expiry,
      });
      setAmountText("");
      setLabel("");
      setExpiryIndex(0);
      onCreated(created);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="card space-y-4" aria-label="Request a payment">
      <h2 className="text-sm font-semibold text-white">Request a payment</h2>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="request-amount">
            Amount (SKL)
          </label>
          <input
            id="request-amount"
            type="text"
            inputMode="decimal"
            className="input"
            placeholder="0.0000"
            pattern={SKL_AMOUNT_PATTERN}
            value={amountText}
            onChange={(e) => setAmountText(e.target.value)}
            required
            disabled={busy}
          />
        </div>
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="request-expiry">
            Expires
          </label>
          <select
            id="request-expiry"
            className="input"
            value={expiryIndex}
            onChange={(e) => setExpiryIndex(Number(e.target.value))}
            disabled={busy}
          >
            {EXPIRY_CHOICES.map((c, i) => (
              <option key={c.label} value={i}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="request-label">
          Label <span className="text-purple-400">(for you; not sent to the payer)</span>
        </label>
        <input
          id="request-label"
          type="text"
          className="input"
          placeholder="Invoice 1042"
          value={label}
          maxLength={MAX_LABEL_CHARS}
          onChange={(e) => setLabel(e.target.value)}
          disabled={busy}
        />
      </div>
      {error && (
        <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-300" role="alert">
          {error}
        </div>
      )}
      <button type="submit" className="btn btn-primary w-full" disabled={!canSubmit}>
        {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
        {busy ? "Creating…" : "Create payment link"}
      </button>
    </form>
  );
}
