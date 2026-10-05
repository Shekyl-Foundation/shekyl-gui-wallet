import { useState } from "react";
import { statusLabel } from "../lib/transactionStatus";
import TransactionHistory, { type HistoryQuery } from "../components/transactions/TransactionHistory";
import TxTools from "../components/transactions/TxTools";
import type { ReceiveAttributionKind, TransferDirection, TransferState } from "../types/transfers";

/** The contract's `direction` filter, as the page offers it. */
const DIRECTIONS: readonly { value: TransferDirection | undefined; label: string }[] = [
  { value: undefined, label: "All" },
  { value: "INCOMING", label: "Received" },
  { value: "OUTGOING", label: "Sent" },
];

/** The contract's `state` filter; the labels are the same ones the rows show. */
const STATES: readonly TransferState[] = [
  "PENDING",
  "CONFIRMED",
  "SPENT",
  "UNSPENDABLE",
  "FAILED",
  "DROPPED",
  "ABANDONED",
];

/** The contract's `attribution` filter: how a receive matched your payment requests. */
const ATTRIBUTIONS: readonly { value: ReceiveAttributionKind; label: string }[] = [
  { value: "MATCHED", label: "Paid a request" },
  { value: "MANUAL_MATCH", label: "Linked by hand" },
  { value: "UNATTRIBUTED", label: "No request" },
  { value: "LABEL_UNKNOWN", label: "Unknown request" },
  { value: "DISPUTED", label: "Disputed" },
];

/** The select's value, or `undefined` for "any". A string outside the list is any. */
function selectedOf<T extends string>(options: readonly T[], value: string): T | undefined {
  for (const option of options) {
    if (option === value) return option;
  }
  return undefined;
}

/** A typed block height, or `undefined` when the field is empty or not a whole number. */
function selectedHeight(value: string): number | undefined {
  if (!/^[0-9]+$/.test(value)) return undefined;
  const height = Number(value);
  return Number.isSafeInteger(height) ? height : undefined;
}

/** Filter controls for the contract's `get_transfers` filters; the history panel does the rest. */
export default function Transactions() {
  const [direction, setDirection] = useState<TransferDirection | undefined>(undefined);
  const [state, setState] = useState<TransferState | undefined>(undefined);
  const [sinceHeightText, setSinceHeightText] = useState("");
  // Committed on blur or Enter, not per keystroke: every query lists the
  // whole ledger, and a half-typed height is not a filter anyone asked for.
  const [sinceHeight, setSinceHeight] = useState<number | undefined>(undefined);
  const [attribution, setAttribution] = useState<ReceiveAttributionKind | undefined>(undefined);
  // Attribution exists on receives only: the control leaves with the "Sent"
  // tab, and its value with it, so a send list is never filtered to nothing.
  const attributionOffered = direction !== "OUTGOING";
  const query: HistoryQuery = {
    direction,
    state,
    sinceHeight,
    attribution: attributionOffered ? attribution : undefined,
  };

  function commitSinceHeight() {
    setSinceHeight(selectedHeight(sinceHeightText));
  }

  function chooseDirection(next: TransferDirection | undefined) {
    setDirection(next);
    if (next === "OUTGOING") setAttribution(undefined);
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-bold text-white">Transactions</h1>
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex gap-1 rounded-lg bg-purple-800/60 p-1" role="tablist" aria-label="Direction">
            {DIRECTIONS.map((d) => (
              <button
                key={d.label}
                type="button"
                role="tab"
                aria-selected={direction === d.value}
                onClick={() => chooseDirection(d.value)}
                className={`rounded-md px-2 py-1 text-[11px] font-semibold ${
                  direction === d.value ? "bg-gold-500/15 text-gold-400" : "text-purple-300 hover:text-white"
                }`}
              >
                {d.label}
              </button>
            ))}
          </div>
          <select
            aria-label="State"
            className="input w-auto py-1 text-[11px]"
            value={state ?? ""}
            onChange={(e) => setState(selectedOf(STATES, e.target.value))}
          >
            <option value="">Any state</option>
            {STATES.map((s) => (
              <option key={s} value={s}>
                {statusLabel(s)}
              </option>
            ))}
          </select>
          {attributionOffered && (
            <select
              aria-label="Request"
              className="input w-auto py-1 text-[11px]"
              value={attribution ?? ""}
              onChange={(e) =>
                setAttribution(
                  selectedOf(
                    ATTRIBUTIONS.map((a) => a.value),
                    e.target.value,
                  ),
                )
              }
            >
              <option value="">Any request</option>
              {ATTRIBUTIONS.map((a) => (
                <option key={a.value} value={a.value}>
                  {a.label}
                </option>
              ))}
            </select>
          )}
          <input
            aria-label="From block"
            inputMode="numeric"
            pattern="[0-9]*"
            placeholder="From block"
            title="Only transactions confirmed at or after this block height. Sends not yet on chain stay listed."
            className="input w-28 py-1 text-[11px]"
            value={sinceHeightText}
            onChange={(e) => setSinceHeightText(e.target.value.trim())}
            onBlur={commitSinceHeight}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitSinceHeight();
            }}
          />
        </div>
      </div>

      <TxTools />
      <TransactionHistory query={query} />
    </div>
  );
}
