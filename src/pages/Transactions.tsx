import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowUpRight, ArrowDownLeft, ShieldCheck } from "lucide-react";

interface TxInfo {
  hash: string;
  amount: number;
  fee: number;
  height: number;
  timestamp: number;
  direction: string;
  confirmed: boolean;
  pqc_protected: boolean;
}

function atomicToSkl(atomic: number): string {
  return (atomic / 1e12).toFixed(4);
}

export default function Transactions() {
  const [txs, setTxs] = useState<TxInfo[]>([]);

  useEffect(() => {
    invoke<TxInfo[]>("get_transactions", { offset: 0, limit: 50 })
      .then(setTxs)
      .catch(() => {});
  }, []);

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-bold text-white">Transactions</h1>

      {txs.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-purple-300">No transactions yet</p>
          <p className="mt-1 text-xs text-purple-400">
            Send or receive SKL to see your transaction history.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {txs.map((tx) => (
            <div
              key={tx.hash}
              className="card flex items-center gap-4 py-3"
            >
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
                      title="Output protected by FCMP++ membership proof with post-quantum signatures"
                    >
                      <ShieldCheck className="h-2.5 w-2.5" />
                      PQC
                    </span>
                  )}
                </div>
                <p className="text-xs text-purple-400">
                  Block {tx.height.toLocaleString()}
                </p>
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
                {!tx.confirmed && (
                  <span className="text-[10px] text-amber-400">Pending</span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
