import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ArrowDownLeft, ShieldCheck } from "lucide-react";
import { statusClass, statusLabel, statusTitle } from "../lib/transactionStatus";
import { atomicAmount, formatSkl } from "../lib/format";
import type { Transfer, Transfers } from "../types/transfers";

/** Poll so pending → confirmed (and failed/dropped) updates without remount. */
const REFRESH_MS = 15_000;

function loadErrorMessage(err: unknown): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message.trim()) return err.message;
  return "Could not load transactions. Try again, or reopen the wallet if this keeps happening.";
}

export default function Transactions() {
  const [txs, setTxs] = useState<Transfer[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  /** Monotonic generation so overlapping loads discard stale results. */
  const loadGen = useRef(0);

  const load = useCallback(async () => {
    const gen = ++loadGen.current;
    try {
      const { transfers } = await invoke<Transfers>("get_transfers");
      if (gen !== loadGen.current) return;
      setTxs(transfers);
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
                <div className="flex items-center gap-2">
                  <p className="font-mono text-xs text-purple-300">
                    {tx.tx_hash.slice(0, 16)}...
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
                  {tx.block_height != null && tx.block_height > 0 && (
                    <span>Block {tx.block_height.toLocaleString()}</span>
                  )}
                  {tx.timestamp > 0 && (
                    <span>
                      {new Date(tx.timestamp * 1000).toLocaleDateString()}
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
                  title={statusTitle(tx.state)}
                >
                  {statusLabel(tx.state)}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
