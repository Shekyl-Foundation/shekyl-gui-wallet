import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QrCode } from "lucide-react";
import { formatSkl } from "../../lib/format";
import type {
  PaymentRequest,
  PaymentRequestFilter,
  PaymentRequestState,
  PaymentRequests,
  PaymentUriResult,
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

/** Poll so a request flips to Paid when the scan matches it, without remount. */
const REFRESH_MS = 15_000;

interface PaymentRequestListProps {
  /** Bumped by the page when a request is created, to refetch at once. */
  version: number;
  /** Show a listed request's link again (the contract's `make_uri`). */
  onShowLink: (request: PaymentRequest, uri: string) => void;
}

function whenLabel(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export default function PaymentRequestList({ version, onShowLink }: PaymentRequestListProps) {
  const [filter, setFilter] = useState<PaymentRequestFilter>("ALL");
  const [rows, setRows] = useState<PaymentRequest[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const { payment_requests } = await invoke<PaymentRequests>("list_payment_requests", { filter });
      setRows(payment_requests);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [filter]);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => void load(), REFRESH_MS);
    return () => window.clearInterval(id);
  }, [load, version]);

  async function showLink(r: PaymentRequest) {
    try {
      const { uri } = await invoke<PaymentUriResult>("make_uri", {
        amount: r.amount,
        label: r.label || undefined,
        rid: r.id,
        expiry: r.expiry,
      });
      onShowLink(r, uri);
    } catch (e) {
      setError(String(e));
    }
  }

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
              onClick={() => setFilter(f.filter)}
              className={`rounded-md px-2 py-1 text-[11px] font-semibold ${
                filter === f.filter ? "bg-gold-500/15 text-gold-400" : "text-purple-300 hover:text-white"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 p-3 text-xs text-red-300" role="alert">
          {error}
        </div>
      )}

      {rows === null ? (
        <p className="text-xs text-purple-400">Loading…</p>
      ) : rows.length === 0 ? (
        <p className="text-xs text-purple-400">No payment requests yet.</p>
      ) : (
        <ul className="space-y-1">
          {rows.map((r) => {
            const meta = STATE_META[r.state];
            return (
              <li
                key={r.id}
                className="flex items-center justify-between gap-3 rounded-lg bg-purple-900/40 px-3 py-2 text-xs"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium text-white">{r.label || `Request ${r.id}`}</p>
                  <p className="text-[10px] text-purple-400">
                    {whenLabel(r.created_at)}
                    {r.expiry !== undefined && r.state === "PENDING" && ` · expires ${whenLabel(r.expiry)}`}
                  </p>
                </div>
                <span className="font-mono text-gold-400">{formatSkl(r.amount)} SKL</span>
                <span className={`w-28 text-right ${meta.className}`}>{meta.label}</span>
                {r.state === "PENDING" && (
                  <button
                    type="button"
                    onClick={() => void showLink(r)}
                    className="btn-ghost rounded-md p-1.5"
                    title="Show payment link"
                    aria-label={`Show payment link for ${r.label || `request ${r.id}`}`}
                  >
                    <QrCode className="h-4 w-4" />
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
