import { ShieldAlert, AlertOctagon } from "lucide-react";

interface Violation {
  invariantId: number;
  invariantName: string;
  reporterIndex: number;
  intentHash: string;
  timestamp: number;
}

interface ViolationAlertProps {
  violations: Violation[];
}

const INVARIANT_LABELS: Record<number, string> = {
  1: "SpendIntent validation (I1)",
  2: "Chain state fingerprint (I2)",
  3: "FCMP++ proof verification (I3)",
  4: "BP+ deterministic match (I4)",
  5: "Prover assignment (I5)",
  6: "Assembly consensus (I6)",
  7: "Receive-time validation (I7)",
};

export default function ViolationAlert({ violations }: ViolationAlertProps) {
  if (violations.length === 0) return null;

  return (
    <div className="rounded-xl border border-red-500/40 bg-red-500/5 p-4 space-y-3">
      <div className="flex items-center gap-2">
        <ShieldAlert className="h-5 w-5 text-red-400" />
        <h3 className="text-sm font-semibold text-red-300">
          Invariant Violations ({violations.length})
        </h3>
      </div>

      <div className="space-y-2">
        {violations.map((v, i) => (
          <div
            key={i}
            className="flex items-start gap-2 rounded-lg border border-red-500/20 bg-red-950/30 p-3"
          >
            <AlertOctagon className="mt-0.5 h-4 w-4 shrink-0 text-red-500" />
            <div className="min-w-0 space-y-1">
              <div className="text-sm font-medium text-red-200">
                {INVARIANT_LABELS[v.invariantId] ?? `Unknown (${v.invariantId})`}
              </div>
              <div className="text-xs text-red-400">
                Reported by participant {v.reporterIndex + 1} ·{" "}
                {new Date(v.timestamp * 1000).toLocaleTimeString()}
              </div>
              <code className="block truncate text-xs text-red-500/70">
                Intent: {v.intentHash.slice(0, 16)}...
              </code>
            </div>
          </div>
        ))}
      </div>

      <p className="text-xs text-red-500">
        Signing has been automatically aborted. Investigate before retrying.
      </p>
    </div>
  );
}
