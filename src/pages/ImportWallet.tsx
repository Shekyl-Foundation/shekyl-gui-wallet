import { useState, useCallback, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  Key,
  FileText,
  Eye,
  EyeOff,
  Loader2,
  Check,
} from "lucide-react";
import { useWallet } from "../context/useWallet";
import type { WalletProgress } from "../types/daemon";
import WalletDirAdvanced from "../components/WalletDirAdvanced";

type Tab = "seed" | "keys";

type RestoreStage =
  | "idle"
  | "scanning"
  | "rederiving_pqc_keys"
  | "building_tree"
  | "complete";

const RESTORE_STAGE_LABELS: Record<RestoreStage, string> = {
  idle: "",
  scanning: "Scanning blockchain...",
  rederiving_pqc_keys: "Rederiving quantum-safe keys...",
  building_tree: "Building FCMP++ tree state...",
  complete: "Restore complete",
};

const RESTORE_STAGES: RestoreStage[] = [
  "scanning",
  "rederiving_pqc_keys",
  "building_tree",
  "complete",
];

export default function ImportWallet() {
  const navigate = useNavigate();
  const { importFromSeed, importFromKeys, setPhase } = useWallet();

  const [tab, setTab] = useState<Tab>("seed");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [restoreStage, setRestoreStage] = useState<RestoreStage>("idle");
  const [progressDetail, setProgressDetail] = useState("");
  const stageTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [name, setName] = useState("Restored Wallet");
  const [password, setPassword] = useState("");
  const [restoreHeight, setRestoreHeight] = useState("");

  const [seed, setSeed] = useState("");

  const [address, setAddress] = useState("");
  const [spendKey, setSpendKey] = useState("");
  const [viewKey, setViewKey] = useState("");

  useEffect(() => {
    if (restoreStage === "idle" || restoreStage === "complete") return;
    const unlistenPromise = listen<WalletProgress>("wallet-progress", (event) => {
      const { event_type, current, total } = event.payload;
      if (event_type === "pqc_rederivation") {
        setRestoreStage("rederiving_pqc_keys");
        setProgressDetail(`${current} / ${total} outputs`);
      } else if (event_type === "fcmp_precompute") {
        setRestoreStage("building_tree");
        setProgressDetail(`${current} / ${total} outputs`);
      }
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [restoreStage]);

  useEffect(() => {
    return () => {
      if (stageTimer.current) clearTimeout(stageTimer.current);
    };
  }, []);

  const handleImportSeed = useCallback(async () => {
    setError(null);
    setLoading(true);
    setRestoreStage("scanning");
    setProgressDetail("");
    try {
      await importFromSeed(
        name.trim(),
        seed.trim(),
        password,
        "English",
        restoreHeight ? parseInt(restoreHeight, 10) : 0,
      );

      setRestoreStage("complete");
      stageTimer.current = setTimeout(() => {
        setPhase("ready");
      }, 1500);
    } catch (e) {
      setRestoreStage("idle");
      setError(String(e));
      setLoading(false);
    }
  }, [importFromSeed, name, seed, password, restoreHeight, setPhase]);

  const handleImportKeys = useCallback(async () => {
    setError(null);
    setLoading(true);
    setRestoreStage("scanning");
    setProgressDetail("");
    try {
      await importFromKeys(
        name.trim(),
        address.trim(),
        spendKey.trim(),
        viewKey.trim(),
        password,
        "English",
        restoreHeight ? parseInt(restoreHeight, 10) : 0,
      );

      setRestoreStage("complete");
      stageTimer.current = setTimeout(() => {
        setPhase("ready");
      }, 1500);
    } catch (e) {
      setRestoreStage("idle");
      setError(String(e));
      setLoading(false);
    }
  }, [
    importFromKeys,
    name,
    address,
    spendKey,
    viewKey,
    password,
    restoreHeight,
    setPhase,
  ]);

  const seedWordCount = seed
    .trim()
    .split(/\s+/)
    .filter(Boolean).length;
  const canSubmitSeed =
    name.trim().length > 0 && password.length >= 8 && seedWordCount === 25;
  const canSubmitKeys =
    name.trim().length > 0 &&
    password.length >= 8 &&
    address.trim().length > 0 &&
    viewKey.trim().length > 0;

  const stageProgress =
    restoreStage === "idle"
      ? 0
      : ((RESTORE_STAGES.indexOf(restoreStage) + 1) / RESTORE_STAGES.length) *
        100;

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-purple-900">
      <div className="mx-auto w-full max-w-lg space-y-6 px-6">
        {/* Header */}
        <div className="flex items-center gap-3">
          <button
            onClick={() => navigate("/")}
            className="rounded-lg p-2 text-purple-300 hover:bg-purple-800"
            disabled={loading}
          >
            <ArrowLeft className="h-4 w-4" />
          </button>
          <h1 className="text-xl font-bold text-white">Import Wallet</h1>
        </div>

        {/* Restore progress overlay */}
        {restoreStage !== "idle" && (
          <div className="card space-y-4">
            <div className="space-y-3">
              {RESTORE_STAGES.map((stage) => {
                const currentIdx = RESTORE_STAGES.indexOf(restoreStage);
                const stageIdx = RESTORE_STAGES.indexOf(stage);
                const isActive = stage === restoreStage;
                const isDone = stageIdx < currentIdx;

                return (
                  <div key={stage} className="flex items-center gap-3">
                    <div className="flex h-5 w-5 items-center justify-center">
                      {isDone ? (
                        <Check className="h-4 w-4 text-emerald-400" />
                      ) : isActive ? (
                        <Loader2 className="h-4 w-4 animate-spin text-gold-400" />
                      ) : (
                        <div className="h-2 w-2 rounded-full bg-purple-600" />
                      )}
                    </div>
                    <span
                      className={`text-xs ${
                        isActive
                          ? "font-semibold text-white"
                          : isDone
                            ? "text-emerald-300"
                            : "text-purple-400"
                      }`}
                    >
                      {RESTORE_STAGE_LABELS[stage]}
                      {isActive && progressDetail && (
                        <span className="ml-1 font-normal text-purple-300">
                          ({progressDetail})
                        </span>
                      )}
                    </span>
                  </div>
                );
              })}
            </div>
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-purple-700">
              <div
                className="h-full rounded-full bg-gold-500 transition-all duration-700"
                style={{ width: `${stageProgress}%` }}
              />
            </div>
          </div>
        )}

        {/* Tab switcher */}
        {restoreStage === "idle" && (
          <>
            <div className="flex gap-1 rounded-lg bg-purple-800/60 p-1">
              <button
                onClick={() => setTab("seed")}
                className={`flex flex-1 items-center justify-center gap-2 rounded-md px-4 py-2 text-xs font-semibold transition-colors ${
                  tab === "seed"
                    ? "bg-gold-500/15 text-gold-400"
                    : "text-purple-300 hover:text-white"
                }`}
              >
                <FileText className="h-3.5 w-3.5" />
                Seed Phrase
              </button>
              <button
                onClick={() => setTab("keys")}
                className={`flex flex-1 items-center justify-center gap-2 rounded-md px-4 py-2 text-xs font-semibold transition-colors ${
                  tab === "keys"
                    ? "bg-gold-500/15 text-gold-400"
                    : "text-purple-300 hover:text-white"
                }`}
              >
                <Key className="h-3.5 w-3.5" />
                Private Keys
              </button>
            </div>

            {error && (
              <div className="rounded-lg border border-red-500/40 bg-red-900/30 p-3 text-xs text-red-200">
                {error}
              </div>
            )}

            <div className="card space-y-4">
              {/* Wallet name */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">
                  Wallet Name
                </label>
                <input
                  type="text"
                  className="input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Restored Wallet"
                />
                <p className="text-[11px] text-purple-400">
                  Spaces are converted to underscores on disk, so "My Wallet"
                  becomes <code>My_Wallet</code>.
                </p>
              </div>

              {/* Seed tab */}
              {tab === "seed" && (
                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-purple-200">
                    25-Word Seed Phrase
                  </label>
                  <textarea
                    className="input min-h-[100px] resize-none font-mono text-sm"
                    value={seed}
                    onChange={(e) => setSeed(e.target.value)}
                    placeholder="Enter your 25-word mnemonic seed phrase, separated by spaces"
                    spellCheck={false}
                    autoComplete="off"
                  />
                  <p className="text-[10px] text-purple-400">
                    {seedWordCount}/25 words
                  </p>
                </div>
              )}

              {/* Keys tab */}
              {tab === "keys" && (
                <>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-purple-200">
                      Address
                    </label>
                    <input
                      type="text"
                      className="input font-mono text-xs"
                      value={address}
                      onChange={(e) => setAddress(e.target.value)}
                      placeholder="shekyl1..."
                      spellCheck={false}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-purple-200">
                      Spend Key
                    </label>
                    <input
                      type="text"
                      className="input font-mono text-xs"
                      value={spendKey}
                      onChange={(e) => setSpendKey(e.target.value)}
                      placeholder="64-character hex string (leave empty for view-only)"
                      spellCheck={false}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-purple-200">
                      View Key
                    </label>
                    <input
                      type="text"
                      className="input font-mono text-xs"
                      value={viewKey}
                      onChange={(e) => setViewKey(e.target.value)}
                      placeholder="64-character hex string"
                      spellCheck={false}
                    />
                  </div>
                </>
              )}

              {/* Password */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">
                  New Password
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
              </div>

              {/* Restore height */}
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-purple-200">
                  Restore Height{" "}
                  <span className="text-purple-400">(optional)</span>
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
                  Block height to start scanning from. Leave at 0 to scan the
                  entire blockchain (slower but safest).
                </p>
              </div>

              {/* Submit */}
              <button
                onClick={tab === "seed" ? handleImportSeed : handleImportKeys}
                disabled={
                  loading ||
                  (tab === "seed" ? !canSubmitSeed : !canSubmitKeys)
                }
                className="btn btn-primary w-full"
              >
                {loading
                  ? "Importing..."
                  : tab === "seed"
                    ? "Restore from Seed"
                    : "Import from Keys"}
              </button>

              <WalletDirAdvanced />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
