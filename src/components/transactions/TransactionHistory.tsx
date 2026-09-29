import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ArrowDownLeft, Tag } from "lucide-react";
import { statusClass, statusLabel, statusTitle } from "../../lib/transactionStatus";
import { atomicAmount, formatSkl } from "../../lib/format";
import type {
  ReceiveAttribution,
  ReceiveAttributionKind,
  Transfer,
  TransferDirection,
  TransferState,
  Transfers,
} from "../../types/transfers";

/** Poll so pending → confirmed (and failed/dropped) updates without remount. */
const REFRESH_MS = 15_000;

/** The filters of one `get_transfers` call. `undefined` on a leg means any. */
export interface HistoryQuery {
  direction: TransferDirection | undefined;
  state: TransferState | undefined;
  /** Inclusion-height watermark: rows mined below it are left out; unmined sends stay. */
  sinceHeight: number | undefined;
  /** Receives by how they matched a payment request; any value excludes sends. */
  attribution: ReceiveAttributionKind | undefined;
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

function sameQuery(left: HistoryQuery, right: HistoryQuery): boolean {
  return (
    left.direction === right.direction &&
    left.state === right.state &&
    left.sinceHeight === right.sinceHeight &&
    left.attribution === right.attribution
  );
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

/** An empty list under no filter is a wallet with no history; under any filter it is a miss. */
function emptyHistoryCopy(query: HistoryQuery): { title: string; detail: string } {
  const unfiltered =
    query.direction === undefined &&
    query.state === undefined &&
    query.sinceHeight === undefined &&
    query.attribution === undefined;
  if (unfiltered) {
    return {
      title: "No transactions yet",
      detail: "Send or receive SKL to see your transaction history.",
    };
  }
  return {
    title: "No matching transactions",
    detail: "Nothing in this wallet matches these filters.",
  };
}

function loadErrorMessage(err: unknown): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message.trim()) return err.message;
  return "Could not load transactions. Try again, or reopen the wallet if this keeps happening.";
}

/**
 * The transfer history for one query: owns the fetch (the contract's
 * `get_transfers` with the query's filters, sent to Rust — never a shown
 * list filtered locally), the poll, the retry, and every fail-closed state.
 * The page composes it beneath the filter controls (rule 27).
 */
export default function TransactionHistory({ query }: { query: HistoryQuery }) {
  const { direction, state, sinceHeight, attribution } = query;
  const [view, setView] = useState<HistoryView>({ kind: "loading", query });
  /** Monotonic generation so overlapping loads discard stale results. */
  const loadGen = useRef(0);

  const load = useCallback(
    async (reason: "query" | "refresh" | "retry") => {
      const gen = ++loadGen.current;
      const requested: HistoryQuery = { direction, state, sinceHeight, attribution };
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
          sinceHeight: requested.sinceHeight,
          attribution: requested.attribution,
        });
        if (gen !== loadGen.current) return;
        setView({ kind: "ready", query: requested, transfers });
      } catch (err) {
        if (gen !== loadGen.current) return;
        setView({ kind: "fault", query: requested, message: loadErrorMessage(err) });
      }
    },
    [direction, state, sinceHeight, attribution],
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
    <>
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
    </>
  );
}
