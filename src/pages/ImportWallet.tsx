import { useState, useCallback, useEffect, useRef } from "react";
import { useNavigate } from "react-router";
import { ArrowLeft, Eye, EyeOff, Loader2, Check } from "lucide-react";
import { useWallet } from "../context/useWallet";
import { BIP39_RECOVERY_PHRASE_WORD_COUNT } from "../constants/wallet";
import WalletDirAdvanced from "../components/WalletDirAdvanced";

/**
 * Restore a wallet from its 24-word recovery phrase — the contract's
 * `restore_wallet` (name, password, mnemonic, restore_height). That is the
 * only restore path a Shekyl wallet has: hybrid post-quantum keys are derived
 * from the seed, so there is no separate spend/view key pair to import, and
 * the seed takes no BIP-39 passphrase.
 */
type Restore = "idle" | "restoring" | "complete";

/** How long the "Restore complete" confirmation stays before the wallet opens. */
const COMPLETE_DWELL_MS = 1500;
const MIN_PASSWORD_LENGTH = 8;

export default function ImportWallet() {
  const navigate = useNavigate();
  const { importFromSeed, setPhase } = useWallet();

  const [restore, setRestore] = useState<Restore>("idle");
  const [error, setError] = useState<string | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const dwellTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [name, setName] = useState("Restored Wallet");
  const [seed, setSeed] = useState("");
  const [password, setPassword] = useState("");
  const [restoreHeight, setRestoreHeight] = useState("");

  useEffect(() => {
    return () => {
      if (dwellTimer.current) clearTimeout(dwellTimer.current);
    };
  }, []);

  const handleRestore = useCallback(async () => {
    setError(null);
    setRestore("restoring");
    try {
      await importFromSeed(
        name.trim(),
        seed.trim(),
        password,
        restoreHeight ? parseInt(restoreHeight, 10) : 0,
      );
      setRestore("complete");
      dwellTimer.current = setTimeout(() => {
        // Navigate before flipping phase; see CreateWallet.handleFinish for
        // why (ready-phase routes don't match /import).
        navigate("/", { replace: true });
        setPhase("ready");
      }, COMPLETE_DWELL_MS);
    } catch (e) {
      setRestore("idle");
      setError(String(e));
    }
  }, [importFromSeed, name, seed, password, restoreHeight, navigate, setPhase]);

  const seedWordCount = seed.trim().split(/\s+/).filter(Boolean).length;
  const canSubmit =
    name.trim().length > 0 &&
    password.length >= MIN_PASSWORD_LENGTH &&
    seedWordCount === BIP39_RECOVERY_PHRASE_WORD_COUNT;
  const busy = restore !== "idle";

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-purple-900">
      <div className="mx-auto w-full max-w-lg space-y-6 px-6">
        {/* Header */}
        <div className="flex items-center gap-3">
          <button
            onClick={() => navigate("/")}
            className="rounded-lg p-2 text-purple-300 hover:bg-purple-800"
            disabled={busy}
            aria-label="Back"
          >
            <ArrowLeft className="h-4 w-4" />
          </button>
          <h1 className="text-xl font-bold text-white">Import Wallet</h1>
        </div>

        {busy ? (
          <div className="card flex items-center gap-3 text-sm" role="status">
            {restore === "complete" ? (
              <>
                <Check className="h-4 w-4 text-emerald-400" />
                <span className="text-emerald-300">Restore complete</span>
              </>
            ) : (
              <>
                <Loader2 className="h-4 w-4 animate-spin text-gold-400" />
                <span className="text-white">
                  Restoring your wallet — deriving keys and scanning the chain from your
                  restore height. This can take a while.
                </span>
              </>
            )}
          </div>
        ) : (
          <>
            {error && (
              <div
                className="rounded-lg border border-red-500/40 bg-red-900/30 p-3 text-xs text-red-200"
                role="alert"
              >
                {error}
              </div>
            )}

            <div className="card space-y-4">
              {/* Wallet name */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">Wallet Name</label>
                <input
                  type="text"
                  className="input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Restored Wallet"
                />
                <p className="text-[11px] text-purple-400">
                  Spaces are converted to underscores on disk, so "My Wallet" becomes{" "}
                  <code>My_Wallet</code>.
                </p>
              </div>

              {/* Recovery phrase */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">
                  24-Word Recovery Phrase
                </label>
                <textarea
                  className="input min-h-[100px] resize-none font-mono text-sm"
                  value={seed}
                  onChange={(e) => setSeed(e.target.value)}
                  placeholder="Enter your 24-word recovery phrase, separated by spaces"
                  spellCheck={false}
                  autoComplete="off"
                />
                <p className="text-[10px] text-purple-400">
                  {seedWordCount}/{BIP39_RECOVERY_PHRASE_WORD_COUNT} words
                </p>
              </div>

              {/* Password */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">New Password</label>
                <div className="relative">
                  <input
                    type={showPassword ? "text" : "password"}
                    className="input pr-10"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder={`At least ${MIN_PASSWORD_LENGTH} characters`}
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-purple-400 hover:text-purple-200"
                    aria-label={showPassword ? "Hide password" : "Show password"}
                  >
                    {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </button>
                </div>
              </div>

              {/* Restore height */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">
                  Restore Height <span className="text-purple-400">(optional)</span>
                </label>
                <input
                  type="number"
                  className="input"
                  value={restoreHeight}
                  onChange={(e) => setRestoreHeight(e.target.value)}
                  placeholder="0"
                  min={0}
                />
                <p className="text-[10px] text-purple-400">
                  Block height to start scanning from. Leave at 0 to scan the entire
                  blockchain (slower but safest).
                </p>
              </div>

              <button onClick={handleRestore} disabled={!canSubmit} className="btn btn-primary w-full">
                Restore from Recovery Phrase
              </button>

              <WalletDirAdvanced />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
