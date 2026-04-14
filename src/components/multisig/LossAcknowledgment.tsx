import { useState } from "react";
import { AlertTriangle, CheckCircle2 } from "lucide-react";

interface LossAcknowledgmentProps {
  nTotal: number;
  onAcknowledge: () => void;
  acknowledged: boolean;
}

export default function LossAcknowledgment({
  nTotal,
  onAcknowledge,
  acknowledged,
}: LossAcknowledgmentProps) {
  const [checked, setChecked] = useState(false);
  const lossPct = Math.round(100 / nTotal);

  if (acknowledged) {
    return (
      <div className="flex items-center gap-2 rounded-lg border border-green-500/20 bg-green-500/5 p-3 text-sm text-green-400">
        <CheckCircle2 className="h-4 w-4 shrink-0" />
        1/N loss limitation acknowledged
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-yellow-500/30 bg-yellow-500/5 p-4 space-y-4">
      <div className="flex items-start gap-3">
        <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-yellow-500" />
        <div className="space-y-2">
          <h3 className="text-sm font-semibold text-yellow-300">
            Important: 1/N Loss Limitation
          </h3>
          <p className="text-sm text-yellow-200/80">
            In a {nTotal}-participant group, each member is the designated prover
            for approximately <strong>~{lossPct}%</strong> of group outputs.
            If any single participant permanently loses their keys, the outputs
            they were assigned as prover become permanently unspendable.
          </p>
          <p className="text-sm text-yellow-200/80">
            This is an accepted V3.1 limitation. V4 will eliminate this
            constraint entirely.
          </p>
        </div>
      </div>

      <label className="flex items-start gap-3 cursor-pointer">
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => setChecked(e.target.checked)}
          className="mt-1 h-4 w-4 rounded border-yellow-500/50 bg-yellow-500/10 text-gold-500 focus:ring-gold-400"
        />
        <span className="text-sm text-yellow-200">
          I understand that losing one participant's keys means ~{lossPct}% of
          group funds become permanently unspendable.
        </span>
      </label>

      <button
        onClick={onAcknowledge}
        disabled={!checked}
        className="btn btn-primary w-full disabled:opacity-50"
      >
        Acknowledge and Continue
      </button>
    </div>
  );
}
