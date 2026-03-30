import { useState, useCallback } from "react";
import { Lock, Eye, EyeOff, ChevronDown } from "lucide-react";
import { useWallet } from "../context/useWallet";

export default function Unlock() {
  const { walletFiles, openWallet, setPhase, error: ctxError } = useWallet();

  const [selectedFile, setSelectedFile] = useState(
    walletFiles[0]?.name ?? "",
  );
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);

  const handleUnlock = useCallback(async () => {
    if (!selectedFile || password.length === 0) return;
    setError(null);
    setLoading(true);
    try {
      await openWallet(selectedFile, password);
    } catch (e) {
      const msg = String(e);
      if (
        msg.toLowerCase().includes("password") ||
        msg.toLowerCase().includes("decrypt")
      ) {
        setError("Incorrect password. Please try again.");
      } else {
        setError(msg);
      }
    } finally {
      setLoading(false);
    }
  }, [selectedFile, password, openWallet]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !loading) {
        handleUnlock();
      }
    },
    [handleUnlock, loading],
  );

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-purple-900">
      <div className="mx-auto w-full max-w-sm space-y-6 px-6">
        {/* Icon */}
        <div className="space-y-3 text-center">
          <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-gold-500/20">
            <Lock className="h-7 w-7 text-gold-400" />
          </div>
          <h1 className="text-xl font-bold text-white">Unlock Wallet</h1>
        </div>

        {(error || ctxError) && (
          <div className="rounded-lg border border-red-500/40 bg-red-900/30 p-3 text-xs text-red-200">
            {error || ctxError}
          </div>
        )}

        <div className="card space-y-4">
          {/* Wallet selector (if multiple) */}
          {walletFiles.length > 1 && (
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-purple-200">
                Wallet
              </label>
              <div className="relative">
                <button
                  onClick={() => setShowPicker(!showPicker)}
                  className="input flex w-full items-center justify-between text-left"
                >
                  <span className="text-sm text-white">
                    {selectedFile || "Select a wallet"}
                  </span>
                  <ChevronDown className="h-4 w-4 text-purple-400" />
                </button>
                {showPicker && (
                  <div className="absolute left-0 right-0 top-full z-10 mt-1 rounded-lg border border-purple-600 bg-purple-800 shadow-xl">
                    {walletFiles.map((f) => (
                      <button
                        key={f.path}
                        onClick={() => {
                          setSelectedFile(f.name);
                          setShowPicker(false);
                        }}
                        className={`block w-full px-4 py-2.5 text-left text-sm transition-colors hover:bg-purple-700/60 ${
                          f.name === selectedFile
                            ? "text-gold-400"
                            : "text-white"
                        }`}
                      >
                        {f.name}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Single wallet display */}
          {walletFiles.length === 1 && (
            <div className="text-center">
              <p className="text-xs text-purple-300">Wallet</p>
              <p className="text-sm font-semibold text-white">
                {walletFiles[0].name}
              </p>
            </div>
          )}

          {/* Password */}
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-purple-200">
              Password
            </label>
            <div className="relative">
              <input
                type={showPassword ? "text" : "password"}
                className="input pr-10"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Enter your password"
                autoFocus
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
          </div>

          <button
            onClick={handleUnlock}
            disabled={loading || !selectedFile || password.length === 0}
            className="btn btn-primary w-full"
          >
            {loading ? "Unlocking..." : "Unlock"}
          </button>
        </div>

        {/* Help links */}
        <div className="space-y-2 text-center text-[11px] text-purple-400">
          <p>
            Forgot your password? You can restore your wallet using your 25-word
            seed phrase.
          </p>
          <button
            onClick={() => setPhase("no_wallet")}
            className="text-gold-400 hover:underline"
          >
            Import a different wallet
          </button>
        </div>
      </div>
    </div>
  );
}
