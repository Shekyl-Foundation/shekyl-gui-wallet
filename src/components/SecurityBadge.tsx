import { useNavigate } from "react-router-dom";
import { ShieldCheck, ShieldAlert } from "lucide-react";
import { useDaemon } from "../context/useDaemon";

function compactOutputs(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

export default function SecurityBadge() {
  const { security, pqc } = useDaemon();
  const navigate = useNavigate();

  const connected = security !== null || pqc !== null;
  if (!connected) return null;

  const hasTree = security && security.anonymity_set_size > 0;

  return (
    <button
      onClick={() => navigate("/settings")}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[10px] font-semibold transition-colors hover:brightness-125 ${
        hasTree
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
          : "border-amber-500/30 bg-amber-500/10 text-amber-300"
      }`}
      title={
        hasTree
          ? `3-layer protection active — ${security.anonymity_set_size.toLocaleString()} outputs in anonymity set`
          : pqc
            ? "PQC active — waiting for curve tree data"
            : "Connecting to daemon..."
      }
    >
      {hasTree ? (
        <ShieldCheck className="h-3 w-3" />
      ) : (
        <ShieldAlert className="h-3 w-3" />
      )}
      {hasTree
        ? `3-layer — ${compactOutputs(security.anonymity_set_size)} outputs`
        : pqc
          ? `${pqc.classical} + ${pqc.post_quantum.split(" ")[0]}`
          : "Connecting..."}
    </button>
  );
}
