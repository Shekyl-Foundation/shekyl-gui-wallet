import { useState, useMemo, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  Copy,
  AlertTriangle,
  Eye,
  EyeOff,
} from "lucide-react";
import { useWallet } from "../context/useWallet";
import type { CreateWalletResult } from "../types/wallet";
import WalletDirAdvanced from "../components/WalletDirAdvanced";

type Step = "setup" | "seed" | "confirm" | "done";

export default function CreateWallet() {
  const navigate = useNavigate();
  const { createWallet, setPhase } = useWallet();

  const [step, setStep] = useState<Step>("setup");
  const [name, setName] = useState("My Wallet");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<CreateWalletResult | null>(null);
  const [copied, setCopied] = useState(false);

  // Seed confirmation state
  const [confirmValues, setConfirmValues] = useState<Record<number, string>>(
    {},
  );

  const seedWords = useMemo(
    () => result?.seed.split(" ").filter(Boolean) ?? [],
    [result],
  );

  const challengeIndices = useMemo(() => {
    if (seedWords.length === 0) return [];
    const indices = new Set<number>();
    while (indices.size < 4 && indices.size < seedWords.length) {
      indices.add(Math.floor(Math.random() * seedWords.length));
    }
    return Array.from(indices).sort((a, b) => a - b);
  }, [seedWords.length]);

  const passwordStrength = useMemo(() => {
    if (password.length === 0) return { label: "", color: "" };
    if (password.length < 8)
      return { label: "Too short", color: "text-red-400" };
    let score = 0;
    if (/[a-z]/.test(password)) score++;
    if (/[A-Z]/.test(password)) score++;
    if (/[0-9]/.test(password)) score++;
    if (/[^a-zA-Z0-9]/.test(password)) score++;
    if (password.length >= 12) score++;
    if (score <= 2) return { label: "Weak", color: "text-orange-400" };
    if (score <= 3) return { label: "Fair", color: "text-yellow-400" };
    return { label: "Strong", color: "text-emerald-400" };
  }, [password]);

  const canProceedSetup =
    name.trim().length > 0 &&
    password.length >= 8 &&
    password === confirmPassword;

  const confirmCorrect = useMemo(() => {
    return challengeIndices.every(
      (i) =>
        confirmValues[i]?.toLowerCase().trim() ===
        seedWords[i]?.toLowerCase(),
    );
  }, [challengeIndices, confirmValues, seedWords]);

  const handleCreate = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const res = await createWallet(name.trim(), password);
      setResult(res);
      setStep("seed");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [createWallet, name, password]);

  const handleCopySeed = useCallback(async () => {
    if (!result?.seed) return;
    await navigator.clipboard.writeText(result.seed);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [result]);

  const handleFinish = useCallback(() => {
    setPhase("ready");
  }, [setPhase]);

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-purple-900">
      <div className="mx-auto w-full max-w-lg space-y-6 px-6">
        {/* Header */}
        <div className="flex items-center gap-3">
          {step === "setup" && (
            <button
              onClick={() => navigate("/")}
              className="rounded-lg p-2 text-purple-300 hover:bg-purple-800"
            >
              <ArrowLeft className="h-4 w-4" />
            </button>
          )}
          <h1 className="text-xl font-bold text-white">Create New Wallet</h1>
        </div>

        {/* Step indicator */}
        <div className="flex gap-2">
          {(["setup", "seed", "confirm", "done"] as const).map((s, i) => (
            <div
              key={s}
              className={`h-1 flex-1 rounded-full transition-colors ${
                (["setup", "seed", "confirm", "done"] as const).indexOf(step) >=
                i
                  ? "bg-gold-500"
                  : "bg-purple-700"
              }`}
            />
          ))}
        </div>

        {error && (
          <div className="rounded-lg border border-red-500/40 bg-red-900/30 p-3 text-xs text-red-200">
            {error}
          </div>
        )}

        {/* Step: Setup */}
        {step === "setup" && (
          <div className="card space-y-5">
            <div className="space-y-2">
              <label className="text-xs font-medium text-purple-200">
                Wallet Name
              </label>
              <input
                type="text"
                className="input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Wallet"
                autoFocus
              />
              <p className="text-[11px] text-purple-400">
                Spaces are converted to underscores on disk, so "My Wallet"
                becomes <code>My_Wallet</code>.
              </p>
            </div>

            <div className="space-y-2">
              <label className="text-xs font-medium text-purple-200">
                Password
              </label>
              <div className="relative">
                <input
                  type={showPassword ? "text" : "password"}
                  className="input pr-10"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 8 characters"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-purple-400 hover:text-purple-200"
                >
                  {showPassword ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
              </div>
              {passwordStrength.label && (
                <p className={`text-xs ${passwordStrength.color}`}>
                  {passwordStrength.label}
                </p>
              )}
            </div>

            <div className="space-y-2">
              <label className="text-xs font-medium text-purple-200">
                Confirm Password
              </label>
              <input
                type={showPassword ? "text" : "password"}
                className="input"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Re-enter your password"
              />
              {confirmPassword.length > 0 && password !== confirmPassword && (
                <p className="text-xs text-red-400">Passwords do not match</p>
              )}
            </div>

            <button
              onClick={handleCreate}
              disabled={!canProceedSetup || loading}
              className="btn btn-primary w-full"
            >
              {loading ? (
                "Creating..."
              ) : (
                <>
                  Continue <ArrowRight className="h-4 w-4" />
                </>
              )}
            </button>

            <WalletDirAdvanced />
          </div>
        )}

        {/* Step: Seed display */}
        {step === "seed" && result && (
          <div className="card space-y-5">
            <div className="flex items-start gap-3 rounded-lg border border-orange-500/40 bg-orange-900/20 p-4">
              <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-orange-400" />
              <div className="text-xs text-orange-200">
                <p className="font-semibold">Write these words down now.</p>
                <p className="mt-1">
                  This 25-word phrase is your only backup. If you lose it, your
                  funds cannot be recovered. Never share it with anyone.
                </p>
              </div>
            </div>

            <div className="grid grid-cols-5 gap-2">
              {seedWords.map((word, i) => (
                <div
                  key={i}
                  className="rounded-lg bg-purple-800/80 px-2 py-2 text-center"
                >
                  <span className="block text-[9px] text-purple-400">
                    {i + 1}
                  </span>
                  <span className="text-xs font-medium text-white">{word}</span>
                </div>
              ))}
            </div>

            <button
              onClick={handleCopySeed}
              className="btn btn-ghost w-full text-xs"
            >
              {copied ? (
                <>
                  <Check className="h-3.5 w-3.5" /> Copied
                </>
              ) : (
                <>
                  <Copy className="h-3.5 w-3.5" /> Copy to clipboard
                </>
              )}
            </button>

            <button
              onClick={() => setStep("confirm")}
              className="btn btn-primary w-full"
            >
              I've saved my seed phrase
              <ArrowRight className="h-4 w-4" />
            </button>
          </div>
        )}

        {/* Step: Confirm seed */}
        {step === "confirm" && (
          <div className="card space-y-5">
            <p className="text-xs text-purple-200">
              Verify you saved your seed correctly by entering these words:
            </p>

            <div className="space-y-3">
              {challengeIndices.map((idx) => (
                <div key={idx} className="space-y-1">
                  <label className="text-xs text-purple-300">
                    Word #{idx + 1}
                  </label>
                  <input
                    type="text"
                    className="input"
                    value={confirmValues[idx] ?? ""}
                    onChange={(e) =>
                      setConfirmValues((prev) => ({
                        ...prev,
                        [idx]: e.target.value,
                      }))
                    }
                    placeholder={`Enter word #${idx + 1}`}
                    autoComplete="off"
                    spellCheck={false}
                  />
                </div>
              ))}
            </div>

            <button
              onClick={() => setStep("done")}
              disabled={!confirmCorrect}
              className="btn btn-primary w-full"
            >
              Verify
              <Check className="h-4 w-4" />
            </button>

            <button
              onClick={() => setStep("seed")}
              className="btn btn-ghost w-full text-xs"
            >
              <ArrowLeft className="h-3.5 w-3.5" />
              Back to seed phrase
            </button>
          </div>
        )}

        {/* Step: Success */}
        {step === "done" && result && (
          <div className="card space-y-5 text-center">
            <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-emerald-500/20">
              <Check className="h-7 w-7 text-emerald-400" />
            </div>
            <h2 className="text-lg font-bold text-white">
              Your wallet is ready
            </h2>
            <div className="space-y-2">
              <p className="text-xs text-purple-300">Address</p>
              <p className="break-all rounded-lg bg-purple-800/80 px-3 py-2 font-mono text-[10px] text-gold-400">
                {result.address}
              </p>
            </div>
            <p className="text-xs text-purple-300">
              Protected by hybrid Ed25519 + ML-DSA-65 signatures.
            </p>
            <button onClick={handleFinish} className="btn btn-primary w-full">
              Open Wallet
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
