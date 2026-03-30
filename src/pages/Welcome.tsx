import { useNavigate } from "react-router-dom";
import { Plus, Download, ShieldCheck } from "lucide-react";
import { useWallet } from "../context/useWallet";

export default function Welcome() {
  const navigate = useNavigate();
  const { error } = useWallet();

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-purple-900">
      <div className="mx-auto max-w-md space-y-8 px-6 text-center">
        {/* Branding */}
        <div className="space-y-3">
          <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-gold-500/20">
            <ShieldCheck className="h-8 w-8 text-gold-400" />
          </div>
          <h1 className="text-3xl font-bold text-white">Shekyl Wallet</h1>
          <p className="text-sm text-purple-200">
            Private. Quantum-Safe. Yours.
          </p>
        </div>

        {error && (
          <div className="rounded-lg border border-red-500/40 bg-red-900/30 p-4 text-left text-xs text-red-200">
            {error}
          </div>
        )}

        {/* Actions */}
        <div className="space-y-4">
          <button
            onClick={() => navigate("/create")}
            className="btn btn-primary w-full gap-3 py-4 text-base"
          >
            <Plus className="h-5 w-5" />
            Create New Wallet
          </button>
          <p className="text-xs text-purple-300">
            Generate a fresh wallet with hybrid PQC protection.
            You'll receive a 25-word recovery phrase.
          </p>

          <div className="relative py-2">
            <div className="absolute inset-0 flex items-center">
              <div className="w-full border-t border-purple-700/60" />
            </div>
            <div className="relative flex justify-center">
              <span className="bg-purple-900 px-4 text-xs text-purple-400">
                or
              </span>
            </div>
          </div>

          <button
            onClick={() => navigate("/import")}
            className="btn btn-secondary w-full gap-3 py-4 text-base"
          >
            <Download className="h-5 w-5" />
            Import Existing Wallet
          </button>
          <p className="text-xs text-purple-300">
            Restore from a seed phrase or private keys.
          </p>
        </div>

        {/* Footer note */}
        <p className="text-[10px] text-purple-400">
          All wallets are automatically protected by hybrid Ed25519 + ML-DSA-65
          post-quantum signatures.
        </p>
      </div>
    </div>
  );
}
