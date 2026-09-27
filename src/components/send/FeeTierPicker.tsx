import { formatSkl } from "../../lib/format";
import type { FeePriorityTier, FeeTierQuote } from "../../types/send";

interface FeeTierPickerProps {
  /** The daemon's tier quotes, or `null` while unavailable. */
  quote: FeeTierQuote | null;
  value: FeePriorityTier;
  onChange: (tier: FeePriorityTier) => void;
  disabled?: boolean;
}

const TIERS: readonly { tier: FeePriorityTier; label: string; hint: string }[] = [
  { tier: "ECONOMY", label: "Economy", hint: "cheapest; a few blocks" },
  { tier: "STANDARD", label: "Standard", hint: "balanced" },
  { tier: "PRIORITY", label: "Priority", hint: "next block" },
];

function quoteFor(quote: FeeTierQuote, tier: FeePriorityTier): string {
  switch (tier) {
    case "ECONOMY":
      return quote.economy_fee;
    case "STANDARD":
      return quote.standard_fee;
    case "PRIORITY":
      return quote.priority_fee;
  }
}

/**
 * The three fee tiers (FL-R17), each with the daemon's quote for a typical
 * transaction. These are estimates for choosing a tier; the exact fee is
 * shown by `ReviewCard` once the transaction is built.
 */
export default function FeeTierPicker({ quote, value, onChange, disabled }: FeeTierPickerProps) {
  return (
    <fieldset className="space-y-2" disabled={disabled}>
      <legend className="text-sm font-medium text-purple-200">Fee priority</legend>
      <div className="grid grid-cols-3 gap-2">
        {TIERS.map(({ tier, label, hint }) => (
          <label
            key={tier}
            className={`cursor-pointer rounded-lg border px-3 py-2 text-xs ${
              value === tier
                ? "border-gold-500 bg-gold-500/10 text-white"
                : "border-purple-600/30 bg-purple-800/30 text-purple-200"
            }`}
          >
            <input
              type="radio"
              name="priority"
              value={tier}
              className="sr-only"
              checked={value === tier}
              onChange={() => onChange(tier)}
            />
            <span className="block font-medium">{label}</span>
            <span className="block text-[10px] opacity-80">{hint}</span>
            <span className="block font-mono text-gold-400">
              {quote ? `≈ ${formatSkl(quoteFor(quote, tier))} SKL` : "estimate unavailable"}
            </span>
          </label>
        ))}
      </div>
      {quote && !disabled && (
        <p className="text-[11px] text-purple-300">
          Estimates for a typical transaction. The exact fee is shown before you confirm.
        </p>
      )}
    </fieldset>
  );
}
