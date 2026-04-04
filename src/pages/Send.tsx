import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send as SendIcon, AlertCircle, ShieldCheck, Loader2 } from "lucide-react";

type ProofStage =
  | "idle"
  | "constructing"
  | "generating_proof"
  | "signing"
  | "broadcasting"
  | "done";

const PROOF_STAGE_LABELS: Record<ProofStage, string> = {
  idle: "",
  constructing: "Constructing transaction...",
  generating_proof: "Generating FCMP++ membership proof...",
  signing: "Applying hybrid PQC signature...",
  broadcasting: "Broadcasting to network...",
  done: "Transaction sent",
};

const STAGE_ORDER: ProofStage[] = [
  "constructing",
  "generating_proof",
  "signing",
  "broadcasting",
  "done",
];

export default function Send() {
  const [address, setAddress] = useState("");
  const [amount, setAmount] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [proofStage, setProofStage] = useState<ProofStage>("idle");
  const stageTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (stageTimer.current) clearTimeout(stageTimer.current);
    };
  }, []);

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  function advanceStage(current: ProofStage): ProofStage {
    const idx = STAGE_ORDER.indexOf(current);
    if (idx < 0 || idx >= STAGE_ORDER.length - 1) return current;
    return STAGE_ORDER[idx + 1];
  }

  async function handleSend(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSending(true);
    setProofStage("constructing");

    const progressTimer = setTimeout(() => setProofStage("generating_proof"), 400);

    try {
      await invoke("transfer", {
        address,
        amount: Math.round(parseFloat(amount) * 1e12),
      });
      clearTimeout(progressTimer);
      setProofStage("signing");
      stageTimer.current = setTimeout(() => {
        setProofStage("broadcasting");
        stageTimer.current = setTimeout(() => {
          setProofStage("done");
          stageTimer.current = setTimeout(() => {
            setProofStage("idle");
            setSending(false);
            setAddress("");
            setAmount("");
          }, 2000);
        }, 500);
      }, 300);
    } catch (err) {
      clearTimeout(progressTimer);
      setError(String(err));
      setProofStage("idle");
      setSending(false);
    }
  }

  const stageProgress =
    proofStage === "idle"
      ? 0
      : ((STAGE_ORDER.indexOf(proofStage) + 1) / STAGE_ORDER.length) * 100;

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Send SKL</h1>

      <form onSubmit={handleSend} className="card space-y-5">
        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Recipient Address
          </label>
          <input
            type="text"
            className="input font-mono text-sm"
            placeholder="shekyl1..."
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            required
            disabled={sending}
          />
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Amount (SKL)
          </label>
          <input
            type="number"
            className="input"
            placeholder="0.0000"
            step="0.0001"
            min="0"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            required
            disabled={sending}
          />
        </div>

        {error && (
          <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
            {error}
          </div>
        )}

        {proofStage !== "idle" && proofStage !== "done" && (
          <div className="space-y-2 rounded-lg border border-purple-600/50 bg-purple-800/40 p-3">
            <div className="flex items-center gap-2 text-xs text-purple-200">
              <Loader2 className="h-3.5 w-3.5 animate-spin text-gold-400" />
              {PROOF_STAGE_LABELS[proofStage]}
            </div>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-purple-700">
              <div
                className="h-full rounded-full bg-gold-500 transition-all duration-500"
                style={{ width: `${stageProgress}%` }}
              />
            </div>
            {proofStage === "generating_proof" && (
              <p className="flex items-center gap-1 text-[10px] text-emerald-300">
                <ShieldCheck className="h-2.5 w-2.5" />
                Full-chain membership proof with post-quantum protection
              </p>
            )}
          </div>
        )}

        {proofStage === "done" && (
          <div className="flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm text-emerald-300">
            <ShieldCheck className="h-4 w-4" />
            Transaction sent with FCMP++ protection
          </div>
        )}

        <button
          type="submit"
          disabled={sending}
          className="btn btn-primary w-full"
        >
          <SendIcon className="h-4 w-4" />
          {sending ? "Sending..." : "Send"}
        </button>
      </form>
    </div>
  );
}
