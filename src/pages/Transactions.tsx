import { useState } from "react";
import { statusLabel } from "../lib/transactionStatus";
import TransactionHistory, { type HistoryQuery } from "../components/transactions/TransactionHistory";
import type { TransferDirection, TransferState } from "../types/transfers";

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

/** The select's value, or `undefined` for "any". A string outside {@link STATES} is any. */
function selectedTransferState(value: string): TransferState | undefined {
  for (const state of STATES) {
    if (state === value) return state;
  }
  return undefined;
}

/** Filter controls for the contract's `direction` / `state`; the history panel does the rest. */
export default function Transactions() {
  const [direction, setDirection] = useState<TransferDirection | undefined>(undefined);
  const [state, setState] = useState<TransferState | undefined>(undefined);
  const query: HistoryQuery = { direction, state };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-bold text-white">Transactions</h1>
        <div className="flex items-center gap-2">
          <div className="flex gap-1 rounded-lg bg-purple-800/60 p-1" role="tablist" aria-label="Direction">
            {DIRECTIONS.map((d) => (
              <button
                key={d.label}
                type="button"
                role="tab"
                aria-selected={direction === d.value}
                onClick={() => setDirection(d.value)}
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
            onChange={(e) => setState(selectedTransferState(e.target.value))}
          >
            <option value="">Any state</option>
            {STATES.map((s) => (
              <option key={s} value={s}>
                {statusLabel(s)}
              </option>
            ))}
          </select>
        </div>
      </div>

      <TransactionHistory query={query} />
    </div>
  );
}
