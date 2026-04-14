import { AlertTriangle, CheckCircle2, User } from "lucide-react";

interface ProverAssignment {
  outputIndex: number;
  outputPubkey: string;
  assignedProver: number;
  amount: string;
}

interface ProverViewProps {
  ourIndex: number;
  nTotal: number;
  assignments: ProverAssignment[];
}

export default function ProverView({
  ourIndex,
  nTotal,
  assignments,
}: ProverViewProps) {
  const perProver = new Map<number, { count: number; totalAmount: bigint }>();
  for (const a of assignments) {
    const entry = perProver.get(a.assignedProver) ?? {
      count: 0,
      totalAmount: 0n,
    };
    entry.count += 1;
    entry.totalAmount += BigInt(a.amount);
    perProver.set(a.assignedProver, entry);
  }

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium text-purple-200">
        Prover Responsibility
      </h3>

      <div className="grid gap-2">
        {Array.from({ length: nTotal }, (_, i) => {
          const info = perProver.get(i);
          const isUs = i === ourIndex;
          const pct = assignments.length
            ? Math.round(((info?.count ?? 0) / assignments.length) * 100)
            : 0;

          return (
            <div
              key={i}
              className={`flex items-center gap-3 rounded-lg border p-3 ${
                isUs
                  ? "border-gold-500/30 bg-gold-500/5"
                  : "border-purple-700/30 bg-purple-900/20"
              }`}
            >
              <User
                className={`h-4 w-4 ${isUs ? "text-gold-400" : "text-purple-400"}`}
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span
                    className={`text-sm font-medium ${isUs ? "text-gold-300" : "text-purple-200"}`}
                  >
                    Participant {i + 1}
                    {isUs && " (you)"}
                  </span>
                </div>
                <span className="text-xs text-purple-400">
                  {info?.count ?? 0} outputs · {pct}% of value
                </span>
              </div>
              <div className="text-right">
                {info && info.count > 0 ? (
                  <CheckCircle2 className="h-4 w-4 text-green-400" />
                ) : (
                  <AlertTriangle className="h-4 w-4 text-yellow-500" />
                )}
              </div>
            </div>
          );
        })}
      </div>

      <p className="text-xs text-purple-500">
        Each participant is the designated prover for ~1/N of group outputs.
        If a participant permanently loses their keys, their assigned outputs
        become unspendable.
      </p>
    </div>
  );
}
