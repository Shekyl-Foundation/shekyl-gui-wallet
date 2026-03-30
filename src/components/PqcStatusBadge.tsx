import { useNavigate } from "react-router-dom";
import { ShieldCheck, ShieldAlert } from "lucide-react";
import { useDaemon } from "../context/useDaemon";

export default function PqcStatusBadge() {
  const { pqc } = useDaemon();
  const navigate = useNavigate();

  if (!pqc) return null;

  return (
    <button
      onClick={() => navigate("/help")}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[10px] font-semibold transition-colors hover:brightness-125 ${
        pqc.enabled
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
          : "border-amber-500/30 bg-amber-500/10 text-amber-300"
      }`}
      title={`${pqc.description} — Click for details`}
    >
      {pqc.enabled ? (
        <ShieldCheck className="h-3 w-3" />
      ) : (
        <ShieldAlert className="h-3 w-3" />
      )}
      {pqc.classical} + {pqc.post_quantum}
    </button>
  );
}
