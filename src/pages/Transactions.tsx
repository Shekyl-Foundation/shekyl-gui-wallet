import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ArrowDownLeft, Tag } from "lucide-react";
import { statusClass, statusLabel, statusTitle } from "../lib/transactionStatus";
import { atomicAmount, formatSkl } from "../lib/format";
import type {
  ReceiveAttribution,
  Transfer,
  TransferDirection,
  TransferState,
  Transfers,
} from "../types/transfers";

/** Poll so pending → confirmed (and failed/dropped) updates without remount. */
const REFRESH_MS = 15_000;

/** The filters of one `get_transfers` call. `undefined` on a leg means any. */
interface HistoryQuery {
  direction: TransferDirection | undefined;
  state: TransferState | undefined;
}

/**
 * The list and the query that produced it are one value. A render whose
 * selected filters differ from `query` shows loading, so a new tab never
 * paints the previous call's rows.
 */
type HistoryView =
  | { kind: "loading"; query: HistoryQuery }
  | { kind: "ready"; query: HistoryQuery; transfers: Transfer[] }
  | { kind: "fault"; query: HistoryQuery; message: string }
  | { kind: "retrying"; query: HistoryQuery; message: string };

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

const UNFILTERED: HistoryQuery = { direction: undefined, state: undefined };

function sameQuery(left: HistoryQuery, right: HistoryQuery): boolean {
  return left.direction === right.direction && left.state === right.state;
}

/** The select's value, or `undefined` for "any". A string outside {@link STATES} is any. */
function selectedTransferState(value: string): TransferState | undefined {
  for (const state of STATES) {
    if (state === value) return state;
  }
  return undefined;
}

/** How a receive's attribution reads on its row. `UNATTRIBUTED` shows nothing. */
function attributionLabel(attribution: ReceiveAttribution): string | null {
  switch (attribution.kind) {
    case "MATCHED":
      return `Request ${attribution.request_id}`;
    case "MANUAL_MATCH":
      return `Request ${attribution.request_id} (matched by you)`;
    case "LABEL_UNKNOWN":
      return "Unrecognised payment reference";
    case "DISPUTED":
      return "Disputed";
    case "UNATTRIBUTED":
      return null;
  }
}

function emptyHistoryCopy(query: HistoryQuery): { title: string; detail: string } {
  if (query.direction === undefined && query.state === undefined) {
    return {
      title: "No transactions yet",
      detail: "Send or receive SKL to see your transaction history.",
    };
  }
  return {
    title: "No matching transactions",
    detail: "Nothing in this wallet has that direction and state.",
  };
}

function loadErrorMessage(err: unknown): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message.trim()) return err.message;
  return "Could not load transactions. Try again, or reopen the wallet if this keeps happening.";
}

export default function Transactions() {
  const [direction, setDirection] = useState<TransferDirection | undefined>(undefined);
  const [state, setState] = useState<TransferState | undefined>(undefined);
  const [view, setView] = useState<HistoryView>({ kind: "loading", query: UNFILTERED });
  /** Monotonic generation so overlapping loads discard stale results. */
  const loadGen = useRef(0);

  const query: HistoryQuery = { direction, state };

  const load = useCallback(
    async (reason: "query" | "refresh" | "retry") => {
      const gen = ++loadGen.current;
      const requested: HistoryQuery = { direction, state };
      if (reason === "query") {
        setView({ kind: "loading", query: requested });
      } else if (reason === "retry") {
        setView((current) =>
          current.kind === "fault" || current.kind === "retrying"
            ? { kind: "retrying", query: requested, message: current.message }
            : { kind: "loading", query: requested },
        );
      }
      try {
        const { transfers } = await invoke<Transfers>("get_transfers", {
          direction: requested.direction,
          state: requested.state,
        });
        if (gen !== loadGen.current) return;
        setView({ kind: "ready", query: requested, transfers });
      } catch (err) {
        if (gen !== loadGen.current) return;
        setView({ kind: "fault", query: requested, message: loadErrorMessage(err) });
      }
    },
    [direction, state],
  );

  useEffect(() => {
    void load("query");
    const id = window.setInterval(() => {
      void load("refresh");
    }, REFRESH_MS);
    const onFocus = () => {
      void load("refresh");
    };
    window.addEventListener("focus", onFocus);
    return () => {
      // Invalidate in-flight applies on unmount or query change so a late
      // response cannot paint rows under a different filter.
      loadGen.current += 1;
      window.clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, [load]);

  const visible: HistoryView = sameQuery(view.query, query)
    ? view
    : { kind: "loading", query };
  const faultMessage =
    visible.kind === "fault" || visible.kind === "retrying" ? visible.message : null;
  const transfers = visible.kind === "ready" ? visible.transfers : [];
  const empty = emptyHistoryCopy(query);

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

      {faultMessage && (
        <div className="card border border-red-500/30 bg-red-500/10 py-4 text-center">
          <p className="text-sm text-red-300">{faultMessage}</p>
          <button
            type="button"
            className="mt-3 text-xs font-medium text-purple-200 underline underline-offset-2 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
            disabled={visible.kind === "retrying"}
            onClick={() => {
              void load("retry");
            }}
          >
            {visible.kind === "retrying" ? "Retrying…" : "Try again"}
          </button>
        </div>
      )}

      {!faultMessage && visible.kind === "loading" ? (
        <div className="card py-12 text-center">
          <p className="text-purple-300">Loading transactions…</p>
        </div>
      ) : !faultMessage && visible.kind === "ready" && transfers.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-purple-300">{empty.title}</p>
          <p className="mt-1 text-xs text-purple-400">{empty.detail}</p>
        </div>
      ) : !faultMessage ? (
        <div className="space-y-2" data-testid="transfers">
          {transfers.map((tx) => {
            const attribution =
              tx.direction === "INCOMING" ? attributionLabel(tx.attribution) : null;
            return (
              <div key={tx.id} className="card flex items-center gap-4 py-3">
                <div
                  className={`flex h-8 w-8 items-center justify-center rounded-full ${
                    tx.direction === "INCOMING"
                      ? "bg-emerald-500/20 text-emerald-400"
                      : "bg-red-500/20 text-red-400"
                  }`}
                >
                  {tx.direction === "INCOMING" ? (
                    <ArrowDownLeft className="h-4 w-4" />
                  ) : (
                    <ArrowUpRight className="h-4 w-4" />
                  )}
                </div>
                <div className="flex-1">
                  <p className="font-mono text-xs text-purple-300">
                    {tx.tx_hash.slice(0, 16)}...
                  </p>
                  <div className="flex items-center gap-2 text-xs text-purple-400">
                    {tx.block_height != null && (
                      <span>Block {tx.block_height.toLocaleString()}</span>
                    )}
                    {attribution && (
                      <span className="inline-flex items-center gap-1 text-purple-200" data-testid="attribution">
                        <Tag className="h-3 w-3" />
                        {attribution}
                      </span>
                    )}
                    {atomicAmount(tx.fee) > 0n && tx.direction === "OUTGOING" && (
                      <span className="text-purple-500">
                        Fee: {formatSkl(tx.fee)}
                      </span>
                    )}
                  </div>
                </div>
                <div className="text-right">
                  <p
                    className={`text-sm font-semibold ${
                      tx.direction === "INCOMING" ? "text-emerald-400" : "text-red-400"
                    }`}
                  >
                    {tx.direction === "INCOMING" ? "+" : "-"}
                    {formatSkl(tx.amount)} SKL
                  </p>
                  <span
                    className={`text-[10px] ${statusClass(tx.state)}`}
                    title={statusTitle(
                      tx.state,
                      tx.state === "UNSPENDABLE" ? tx.unspendable_reason : undefined,
                    )}
                  >
                    {statusLabel(tx.state)}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
