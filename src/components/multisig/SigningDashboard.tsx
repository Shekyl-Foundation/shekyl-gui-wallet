import {
  Clock,
  CheckCircle2,
  XCircle,
  Loader2,
  Send,
  AlertTriangle,
} from "lucide-react";

type IntentState =
  | "proposed"
  | "verified"
  | "prover_ready"
  | "signed"
  | "assembled"
  | "broadcast"
  | "rejected"
  | "timed_out";

interface IntentSummary {
  intentHash: string;
  state: IntentState;
  proposerIndex: number;
  sigsCollected: number;
  sigsRequired: number;
  expiresAt: number;
  recipients: { address: string; amount: string }[];
  fee: string;
}

interface SigningDashboardProps {
  intents: IntentSummary[];
  ourIndex: number;
  nowSecs: number;
  onSign?: (intentHash: string) => void;
  onVeto?: (intentHash: string) => void;
}

const STATE_CONFIG: Record<
  IntentState,
  { label: string; color: string; icon: typeof Clock }
> = {
  proposed: { label: "Proposed", color: "text-blue-400", icon: Clock },
  verified: { label: "Verified", color: "text-cyan-400", icon: CheckCircle2 },
  prover_ready: { label: "Prover Ready", color: "text-purple-400", icon: Loader2 },
  signed: { label: "Signed", color: "text-gold-400", icon: CheckCircle2 },
  assembled: { label: "Assembled", color: "text-green-400", icon: Send },
  broadcast: { label: "Broadcast", color: "text-green-300", icon: CheckCircle2 },
  rejected: { label: "Rejected", color: "text-red-400", icon: XCircle },
  timed_out: { label: "Timed Out", color: "text-yellow-500", icon: AlertTriangle },
};

export default function SigningDashboard({
  intents,
  nowSecs,
  onSign,
  onVeto,
}: SigningDashboardProps) {
  const active = intents.filter(
    (i) => !["broadcast", "rejected", "timed_out"].includes(i.state)
  );
  const terminal = intents.filter((i) =>
    ["broadcast", "rejected", "timed_out"].includes(i.state)
  );

  return (
    <div className="space-y-6">
      <h3 className="text-sm font-medium text-purple-200">Signing Dashboard</h3>

      {active.length === 0 && (
        <div className="rounded-lg border border-purple-700/30 bg-purple-900/20 p-6 text-center text-sm text-purple-400">
          No active signing requests
        </div>
      )}

      {active.map((intent) => {
        const cfg = STATE_CONFIG[intent.state];
        const Icon = cfg.icon;
        const remaining = Math.max(0, intent.expiresAt - nowSecs);
        const minutes = Math.floor(remaining / 60);

        return (
          <div
            key={intent.intentHash}
            className="rounded-xl border border-purple-700/40 bg-purple-900/20 p-4 space-y-3"
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Icon className={`h-4 w-4 ${cfg.color}`} />
                <span className={`text-sm font-medium ${cfg.color}`}>{cfg.label}</span>
              </div>
              <span className="text-xs text-purple-500">
                {minutes > 0 ? `${minutes}m remaining` : "Expiring"}
              </span>
            </div>

            <code className="block truncate font-mono text-xs text-purple-400">
              {intent.intentHash}
            </code>

            <div className="text-xs text-purple-300 space-y-1">
              {intent.recipients.map((r, i) => (
                <div key={i} className="flex justify-between">
                  <span className="truncate max-w-[200px]">{r.address}</span>
                  <span className="font-mono">{r.amount}</span>
                </div>
              ))}
              <div className="flex justify-between border-t border-purple-800/30 pt-1">
                <span>Fee</span>
                <span className="font-mono">{intent.fee}</span>
              </div>
            </div>

            <div className="flex items-center justify-between">
              <span className="text-xs text-purple-400">
                Signatures: {intent.sigsCollected}/{intent.sigsRequired}
              </span>
              <span className="text-xs text-purple-500">
                Proposer: participant {intent.proposerIndex + 1}
              </span>
            </div>

            {(intent.state === "verified" || intent.state === "prover_ready") && (
              <div className="flex gap-2">
                {onSign && (
                  <button
                    onClick={() => onSign(intent.intentHash)}
                    className="btn btn-primary flex-1 text-sm"
                  >
                    Sign
                  </button>
                )}
                {onVeto && (
                  <button
                    onClick={() => onVeto(intent.intentHash)}
                    className="btn flex-1 border border-red-500/30 bg-red-500/10 text-sm text-red-400 hover:bg-red-500/20"
                  >
                    Veto
                  </button>
                )}
              </div>
            )}
          </div>
        );
      })}

      {terminal.length > 0 && (
        <details className="group">
          <summary className="cursor-pointer text-xs font-medium text-purple-500 hover:text-purple-300">
            Recent ({terminal.length})
          </summary>
          <div className="mt-2 space-y-2">
            {terminal.map((intent) => {
              const cfg = STATE_CONFIG[intent.state];
              const Icon = cfg.icon;
              return (
                <div
                  key={intent.intentHash}
                  className="flex items-center gap-2 rounded-lg border border-purple-800/20 bg-purple-950/20 p-2.5"
                >
                  <Icon className={`h-3.5 w-3.5 ${cfg.color}`} />
                  <code className="flex-1 truncate font-mono text-xs text-purple-500">
                    {intent.intentHash.slice(0, 24)}...
                  </code>
                  <span className={`text-xs ${cfg.color}`}>{cfg.label}</span>
                </div>
              );
            })}
          </div>
        </details>
      )}
    </div>
  );
}
