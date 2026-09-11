import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Coins,
  Lock,
  TrendingUp,
  ShieldCheck,
  BookOpen,
  KeyRound,
  Loader2,
} from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { useWallet } from "../context/useWallet";
import { formatSklCompact, formatPercent } from "../lib/format";
import type { DrainBalance } from "../types/daemon";
import EmissionGauge from "../components/EmissionGauge";
import ShardIdentityPreview from "../components/staking/ShardIdentityPreview";
import YourStakePanel from "../components/staking/YourStakePanel";

interface StakerStatusInfo {
  staking_enabled: boolean;
  has_stake_engine: boolean;
  bonded_slot_count: number;
  has_pscan: boolean;
}

interface ActivateStakerResult {
  slot: number;
  swept_inputs: number;
  resumed: boolean;
  state: string;
}

/**
 * Staking page — archival participation (GUI-PR0 honesty + GUI-PR3
 * activation + GUI-PR3b staked-balance/outputs read panel).
 *
 * Page owns activation and network stats; personal stake read lives in
 * [`YourStakePanel`] (fetch + fail-closed render). Funding (stake_in) and
 * unbond land in later PRs.
 */
export default function Staking() {
  const { health } = useDaemon();
  const { phase } = useWallet();
  const walletOpen = phase === "ready";

  const [status, setStatus] = useState<StakerStatusInfo | null>(null);
  const [drain, setDrain] = useState<DrainBalance | null>(null);
  const [password, setPassword] = useState("");
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastOutcome, setLastOutcome] = useState<ActivateStakerResult | null>(
    null,
  );

  const refreshStatus = useCallback(() => {
    if (!walletOpen) {
      setStatus(null);
      return;
    }
    invoke<StakerStatusInfo>("get_staker_status")
      .then(setStatus)
      .catch(() => setStatus(null));
  }, [walletOpen]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus, health]);

  // Drainable (P) is a staker-only figure; poll it alongside status, but only
  // once the wallet is an active staker (a non-staker's core read is a plain
  // Ok(0) — no point showing it outside the active panel). A fault or a closed
  // wallet resets to null → the panel renders "—", never a fabricated zero; the
  // transient "syncing" arm is the only non-value render (DS-PR-3, rule 82).
  // The `cancelled` guard (matching Shards.tsx) drops a late-resolving read if
  // the wallet closes / staking is disabled / the component unmounts first, so
  // a stale in-flight value can never re-populate `drain` after the reset.
  useEffect(() => {
    if (!walletOpen || !status?.staking_enabled) {
      setDrain(null);
      return;
    }
    let cancelled = false;
    invoke<DrainBalance>("get_drain_balance")
      .then((d) => {
        if (!cancelled) setDrain(d);
      })
      .catch(() => {
        if (!cancelled) setDrain(null);
      });
    return () => {
      cancelled = true;
    };
  }, [walletOpen, status?.staking_enabled, health]);

  const stakeRatioPct = health ? (health.stake_ratio / 1_000_000) * 100 : 0;
  const emSharePct = health
    ? (health.staker_emission_share_effective / 1_000_000) * 100
    : 0;

  const canActivate =
    walletOpen &&
    status &&
    !status.staking_enabled &&
    password.length > 0 &&
    !activating;

  const onActivate = async () => {
    if (!canActivate) return;
    setActivating(true);
    setError(null);
    setLastOutcome(null);
    try {
      const result = await invoke<ActivateStakerResult>("activate_staker", {
        password,
      });
      setLastOutcome(result);
      setPassword("");
      refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setActivating(false);
    }
  };

  const stakerActive = Boolean(walletOpen && status?.staking_enabled);

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Staking</h1>

      {/* Model narrative */}
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
        <div className="flex items-start gap-3">
          <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
          <div>
            <p className="text-sm font-semibold text-emerald-300">
              Archival staking
            </p>
            <p className="mt-1 text-xs leading-relaxed text-emerald-200/80">
              Staking means becoming an archival participant: your wallet
              activates a staker persona, posts a bond (broadcast is scheduled,
              not instant), and later holds shards as useful work. Principal
              funding and reward recovery ship in follow-up releases.
            </p>
          </div>
        </div>
      </div>

      {/* Activation */}
      <div className="card space-y-4">
        <div className="flex items-center gap-2">
          <KeyRound className="h-4 w-4 text-gold-400" />
          <h2 className="text-sm font-semibold text-purple-200">
            Become a staker
          </h2>
        </div>

        {!walletOpen && (
          <p className="text-xs text-purple-300">
            Open an Engine wallet to activate staking.
          </p>
        )}

        {walletOpen && !status && (
          <p className="text-xs text-purple-300">Checking staker status…</p>
        )}

        {stakerActive && status && (
          <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-100">
            <p className="font-semibold text-emerald-200">Staker active</p>
            <p className="mt-1 text-emerald-100/80">
              Bonded slots: {status.bonded_slot_count}
              {status.has_stake_engine ? " · stake engine running" : ""}
              {status.has_pscan ? " · persona scan running" : ""}
            </p>
            <p className="mt-1 text-emerald-100/80">
              Drainable (P):{" "}
              {drain === null ? (
                // loading or a non-transient fault (the fetch .catch resets to
                // null) → a dash placeholder, never a fabricated zero.
                <span className="text-emerald-100/60">—</span>
              ) : drain.status === "syncing" ? (
                // transient anchor lag → "syncing", the only non-value render;
                // `detail` is the operator-facing reason (tooltip).
                <span className="text-emerald-100/60" title={drain.detail}>
                  Syncing…
                </span>
              ) : (
                <span>{formatSklCompact(drain.spendable)} SKL</span>
              )}
            </p>
            <p className="mt-1 text-emerald-100/70">
              Bond posts may still be pending scheduled broadcast
              (pending_dispatch). Funding the persona and holding shards land
              in later releases.
            </p>
          </div>
        )}

        {walletOpen && status && !status.staking_enabled && (
          <>
            <p className="text-xs text-purple-300">
              Re-enter your wallet password to activate. This re-materializes
              keys for the first bond post. Nothing is broadcast on this step —
              the post is sealed for scheduled dispatch.
            </p>
            <input
              type="password"
              className="input"
              placeholder="Wallet password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={activating}
              autoComplete="current-password"
            />
            {error && <p className="text-xs text-red-300">{error}</p>}
            {lastOutcome && (
              <p className="text-xs text-emerald-200">
                Activation sealed: slot {lastOutcome.slot},{" "}
                {lastOutcome.swept_inputs} funding input(s)
                {lastOutcome.resumed ? " (resumed)" : ""}, state{" "}
                <span className="font-mono">{lastOutcome.state}</span>
              </p>
            )}
            <button
              type="button"
              className="btn btn-primary w-full"
              disabled={!canActivate}
              onClick={() => void onActivate()}
            >
              {activating ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Activating…
                </>
              ) : (
                <>
                  <Lock className="h-4 w-4" />
                  Activate staker
                </>
              )}
            </button>
          </>
        )}
      </div>

      {stakerActive && <YourStakePanel refreshKey={health} />}

      <ShardIdentityPreview />

      {/* Network stats */}
      {health && (
        <div className="card">
          <div className="mb-4 flex items-center gap-2">
            <Coins className="h-4 w-4 text-gold-400" />
            <h2 className="text-sm font-semibold text-purple-200">
              Network stats
            </h2>
          </div>
          <p className="mb-4 text-xs text-purple-300">
            Network-wide daemon metrics, not your personal yield.
          </p>
          <div className="grid grid-cols-4 gap-4">
            <EmissionGauge
              value={stakeRatioPct}
              label="Stake Ratio"
              display={formatPercent(health.stake_ratio)}
            />
            <EmissionGauge
              value={emSharePct}
              label="Emission Share"
              display={formatPercent(health.staker_emission_share_effective)}
            />
            <div className="flex flex-col items-center justify-center gap-1">
              <Lock className="h-5 w-5 text-gold-400" />
              <span className="text-sm font-bold text-white">
                {formatSklCompact(health.total_staked)}
              </span>
              <span className="text-[10px] text-purple-300">Total Staked</span>
            </div>
            <div className="flex flex-col items-center justify-center gap-1">
              <TrendingUp className="h-5 w-5 text-gold-400" />
              <span className="text-sm font-bold text-white">
                {formatSklCompact(health.staker_pool_balance)}
              </span>
              <span className="text-[10px] text-purple-300">Reward Pool</span>
            </div>
          </div>
        </div>
      )}

      {!health && (
        <p className="text-center text-xs text-purple-300">
          Connect to a daemon to see network staking stats
        </p>
      )}

      <div className="rounded-xl border border-purple-600/30 bg-purple-900/40 p-4">
        <div className="flex items-start gap-3">
          <BookOpen className="mt-0.5 h-5 w-5 shrink-0 text-purple-300" />
          <div>
            <p className="text-sm font-semibold text-purple-100">
              Operator notes
            </p>
            <p className="mt-1 text-xs leading-relaxed text-purple-200/80">
              Do not batch activations on a shared schedule. Collateral release
              uses a multi-epoch cooldown. Full guidance: shekyl-core{" "}
              <span className="font-mono text-purple-100">
                STAKER_OPERATOR_GUIDE.md
              </span>
              .
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
