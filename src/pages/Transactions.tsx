import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ArrowDownLeft, ShieldCheck } from "lucide-react";
import {
  statusClass,
  statusLabel,
  statusTitle,
  type TxDirection,
  type TxStatus,
} from "../lib/transactionStatus";

interface TxInfo {
  id: string;
  hash: string;
  amount: number;
  fee: number;
  /** Inclusion height, or null when not on chain. */
  height: number | null;
  timestamp: number;
  direction: TxDirection;
  status: TxStatus;
  pqc_protected: boolean;
}

/** Poll so pending → confirmed (and failed/dropped) updates without remount. */
const REFRESH_MS = 15_000;

function atomicToSkl(atomic: number): string {
  return (atomic / 1e9).toFixed(4);
}

function loadErrorMessage(err: unknown): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message.trim()) return err.message;
  return "Could not load transactions. Try again, or reopen the wallet if this keeps happening.";
}

export default function Transactions() {
  const [txs, setTxs] = useState<TxInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  /** Monotonic generation so overlapping loads discard stale results. */
  const loadGen = useRef(0);

  const load = useCallback(async () => {
    const gen = ++loadGen.current;
    try {
      const rows = await invoke<TxInfo[]>("get_transactions", {
        offset: 0,
        limit: 50,
      });
      if (gen !== loadGen.current) return;
      setTxs(rows);
      setError(null);
    } catch (err) {
      if (gen !== loadGen.current) return;
      setError(loadErrorMessage(err));
    } finally {
      if (gen === loadGen.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => {
      void load();
    }, REFRESH_MS);
    const onFocus = () => {
      void load();
    };
    window.addEventListener("focus", onFocus);
    return () => {
      // Invalidate in-flight applies on unmount so setState is never called
      // after the component is gone (and so a late response cannot win).
      loadGen.current += 1;
      window.clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, [load]);

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-bold text-white">Transactions</h1>

      {error && (
        <div className="card border border-red-500/30 bg-red-500/10 py-4 text-center">
          <p className="text-sm text-red-300">{error}</p>
          <button
            type="button"
            className="mt-3 text-xs font-medium text-purple-200 underline underline-offset-2 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
            disabled={loading}
            onClick={() => {
              // Keep the error card visible with "Retrying…" feedback; loadGen
              // makes double-clicks discard the older in-flight result.
              setLoading(true);
              void load();
            }}
          >
            {loading ? "Retrying…" : "Try again"}
          </button>
        </div>
      )}

      {!error && loading && txs.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-purple-300">Loading transactions…</p>
        </div>
      ) : !error && txs.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-purple-300">No transactions yet</p>
          <p className="mt-1 text-xs text-purple-400">
            Send or receive SKL to see your transaction history.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {txs.map((tx) => (
            <div key={tx.id} className="card flex items-center gap-4 py-3">
              <div
                className={`flex h-8 w-8 items-center justify-center rounded-full ${
                  tx.direction === "in"
                    ? "bg-emerald-500/20 text-emerald-400"
                    : "bg-red-500/20 text-red-400"
                }`}
              >
                {tx.direction === "in" ? (
                  <ArrowDownLeft className="h-4 w-4" />
                ) : (
                  <ArrowUpRight className="h-4 w-4" />
                )}
              </div>
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <p className="font-mono text-xs text-purple-300">
                    {tx.hash.slice(0, 16)}...
                  </p>
                  {tx.pqc_protected && (
                    <span
                      className="inline-flex items-center gap-0.5 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-1.5 py-0.5 text-[9px] font-semibold text-emerald-300"
                      title="Protected by post-quantum signatures"
                    >
                      <ShieldCheck className="h-2.5 w-2.5" />
                      PQC
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-2 text-xs text-purple-400">
                  {tx.height != null && tx.height > 0 && (
                    <span>Block {tx.height.toLocaleString()}</span>
                  )}
                  {tx.timestamp > 0 && (
                    <span>
                      {new Date(tx.timestamp * 1000).toLocaleDateString()}
                    </span>
                  )}
                  {tx.fee > 0 && tx.direction === "out" && (
                    <span className="text-purple-500">
                      Fee: {atomicToSkl(tx.fee)}
                    </span>
                  )}
                </div>
              </div>
              <div className="text-right">
                <p
                  className={`text-sm font-semibold ${
                    tx.direction === "in" ? "text-emerald-400" : "text-red-400"
                  }`}
                >
                  {tx.direction === "in" ? "+" : "-"}
                  {atomicToSkl(tx.amount)} SKL
                </p>
                <span
                  className={`text-[10px] ${statusClass(tx.status)}`}
                  title={statusTitle(tx.status)}
                >
                  {statusLabel(tx.status)}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
