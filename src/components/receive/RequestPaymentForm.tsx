import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2 } from "lucide-react";
import { parseSkl, SKL_AMOUNT_PATTERN } from "../../lib/format";
import { describeError } from "../../lib/errors";
import type { CreatedPaymentRequest } from "../../types/receiving";

const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;
const HOURS_PER_DAY = 24;
const DAYS_PER_WEEK = 7;
const SECONDS_PER_HOUR = MINUTES_PER_HOUR * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY = HOURS_PER_DAY * SECONDS_PER_HOUR;
const SECONDS_PER_WEEK = DAYS_PER_WEEK * SECONDS_PER_DAY;

/** Expiry choices, as durations from now. `none` does not expire. */
const EXPIRY_CHOICES = [
  { id: "none", label: "No expiry", seconds: null },
  { id: "1h", label: "1 hour", seconds: SECONDS_PER_HOUR },
  { id: "24h", label: "24 hours", seconds: SECONDS_PER_DAY },
  { id: "7d", label: "7 days", seconds: SECONDS_PER_WEEK },
] as const;

type ExpiryChoiceId = (typeof EXPIRY_CHOICES)[number]["id"];

/** Same character bound as `MAX_LABEL_CHARS` in `receiving.rs`. The label is written onto the link. */
const MAX_LABEL_CHARS = 256;

interface RequestPaymentFormProps {
  /** Called with the created request; the page shows its URI and QR. */
  onCreated: (created: CreatedPaymentRequest) => void;
}

/**
 * The contract's `create_payment_request`: amount, a label, optional
 * expiry. The label is stored and copied onto the `shekyl:` link, so the
 * payer sees it. Paying the link fills their send form; it does not yet
 * mark this request paid.
 */
export default function RequestPaymentForm({ onCreated }: RequestPaymentFormProps) {
  const [amountText, setAmountText] = useState("");
  const [label, setLabel] = useState("");
  const [expiryId, setExpiryId] = useState<ExpiryChoiceId>("none");
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
    const choice = EXPIRY_CHOICES.find((c) => c.id === expiryId) ?? EXPIRY_CHOICES[0];
    const expiry = choice.seconds === null ? undefined : Math.floor(Date.now() / 1000) + choice.seconds;
    setBusy(true);
    try {
      const created = await invoke<CreatedPaymentRequest>("create_payment_request", {
        label: label.trim(),
        amount: amount.toString(),
        expiry,
      });
      setAmountText("");
      setLabel("");
      setExpiryId("none");
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
            value={expiryId}
            onChange={(e) => {
              const id = e.target.value as ExpiryChoiceId;
              if (EXPIRY_CHOICES.some((c) => c.id === id)) setExpiryId(id);
            }}
            disabled={busy}
          >
            {EXPIRY_CHOICES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="request-label">
          Label <span className="text-purple-400">(on the link; the payer will see it)</span>
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
