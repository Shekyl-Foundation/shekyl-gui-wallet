import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QrCode } from "lucide-react";
import { formatSkl, SKL_DECIMALS } from "../../lib/format";
import { describeError } from "../../lib/errors";
import type {
  PaymentRequest,
  PaymentRequestFilter,
  PaymentRequestState,
  PaymentRequests,
} from "../../types/receiving";

const FILTERS: readonly { filter: PaymentRequestFilter; label: string }[] = [
  { filter: "ALL", label: "All" },
  { filter: "PENDING", label: "Pending" },
  { filter: "MATCHED", label: "Paid" },
];

/** How each contract state reads; every arm distinct (rule 82). */
const STATE_META: Record<PaymentRequestState, { label: string; className: string }> = {
  PENDING: { label: "Awaiting payment", className: "text-amber-300" },
  MATCHED: { label: "Paid", className: "text-emerald-300" },
  EXPIRED: { label: "Expired", className: "text-purple-400" },
  CANCELLED: { label: "Cancelled", className: "text-purple-400" },
};

/**
 * How often to re-read the list. Expiry is classified when the engine
 * reads the row (`state_at`), so a pending request becomes Expired only
 * on a refresh. Paying a link does not mark it Paid yet.
 */
const REFRESH_MS = 15_000;

/**
 * One discriminant. A failed first read is `fault`, never a `loading`
 * line left on screen, and a failed refresh replaces the rows rather
 * than leaving them beside an error (rule 27).
 */
type RequestsLoad =
  | { kind: "loading" }
  | { kind: "ready"; requests: PaymentRequest[] }
  | { kind: "fault"; message: string };

interface PaymentRequestListProps {
  /** Bumped by the page when a request is created, to refetch at once. */
  version: number;
  /** Show the link the list already carried for this stored request. */
  onShowLink: (request: PaymentRequest) => void;
}

function whenLabel(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export default function PaymentRequestList({ version, onShowLink }: PaymentRequestListProps) {
  const [filter, setFilter] = useState<PaymentRequestFilter>("ALL");
  const [load, setLoad] = useState<RequestsLoad>({ kind: "loading" });

  function chooseFilter(next: PaymentRequestFilter) {
    if (next === filter) return;
    setFilter(next);
    // Drop the previous filter's rows immediately. Keeping them under the
    // new tab would show the wrong set while the next read is in flight.
    setLoad({ kind: "loading" });
  }

  useEffect(() => {
    let cancelled = false;
    // A slower read must not paint over a newer one for this same filter
    // (the poll can overlap itself; a filter change cancels via cleanup).
    let generation = 0;

    async function loadRequests() {
      const ticket = ++generation;
      try {
        const { payment_requests } = await invoke<PaymentRequests>("list_payment_requests", { filter });
        if (cancelled || ticket !== generation) return;
        setLoad({ kind: "ready", requests: payment_requests });
      } catch (e) {
        if (cancelled || ticket !== generation) return;
        setLoad({ kind: "fault", message: describeError(e) });
      }
    }

    void loadRequests();
    const id = window.setInterval(() => void loadRequests(), REFRESH_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [filter, version]);

  const requests = load.kind === "ready" ? load.requests : [];

  return (
    <div className="card space-y-3" aria-label="Payment requests">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-white">Payment requests</h2>
        <div className="flex gap-1 rounded-lg bg-purple-800/60 p-1" role="tablist">
          {FILTERS.map((f) => (
            <button
              key={f.filter}
              type="button"
              role="tab"
              aria-selected={filter === f.filter}
              onClick={() => chooseFilter(f.filter)}
              className={`rounded-md px-2 py-1 text-[11px] font-semibold ${
                filter === f.filter ? "bg-gold-500/15 text-gold-400" : "text-purple-300 hover:text-white"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {load.kind === "fault" && (
        <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-300" role="alert">
          {load.message}
        </div>
      )}

      {load.kind === "loading" ? (
        <p className="text-xs text-purple-400">Loading…</p>
      ) : load.kind === "ready" && requests.length === 0 ? (
        <p className="text-xs text-purple-400">No payment requests yet.</p>
      ) : load.kind === "ready" ? (
        <ul className="space-y-1">
          {requests.map((request) => {
            const meta = STATE_META[request.state];
            const name = request.label || `Request ${request.id}`;
            return (
              <li
                key={request.id}
                className="flex items-center justify-between gap-3 rounded-lg bg-purple-900/40 px-3 py-2 text-xs"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium text-white">{name}</p>
                  <p className="text-[10px] text-purple-400">
                    {whenLabel(request.created_at)}
                    {request.expiry !== undefined &&
                      request.state === "PENDING" &&
                      ` · expires ${whenLabel(request.expiry)}`}
                  </p>
                </div>
                <span className="font-mono text-gold-400">{formatSkl(request.amount, SKL_DECIMALS)} SKL</span>
                <span className={`w-28 text-right ${meta.className}`}>{meta.label}</span>
                {request.state === "PENDING" && (
                  <button
                    type="button"
                    onClick={() => onShowLink(request)}
                    className="btn-ghost rounded-md p-1.5"
                    title="Show payment link"
                    aria-label={`Show payment link for ${name}`}
                  >
                    <QrCode className="h-4 w-4" />
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      ) : null}
    </div>
  );
}
