import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Lock } from "lucide-react";
import { formatSklCompact } from "../../lib/format";
import type { StakingView } from "../../types/daemon";

/**
 * Load state for the WI-RPC-1 staking read (GUI-PR3b).
 *
 * One discriminant, not two booleans: loading / ready / fault stay mutually
 * exclusive by construction. Fail-closed (rule 82): a read fault is never
 * rendered as an empty or zero stake panel.
 */
type StakeViewLoad =
  | { kind: "loading" }
  | { kind: "ready"; view: StakingView }
  | { kind: "fault" };

export interface YourStakePanelProps {
  /**
   * Bumps when chain/daemon health refreshes so the panel re-polls.
   * Mount is gated by the parent (`stakerActive`) — this component assumes
   * it is only rendered for an open active staker.
   */
  refreshKey?: unknown;
}

/**
 * "Your stake" panel: bonded principal legs, unspent rewards, staked outputs,
 * and persona-scan frontier. Owns its own fetch; the Staking page only decides
 * whether to mount it.
 *
 * Fetch shape matches Shards.tsx: the effect only schedules the external
 * invoke; setState runs in the promise callbacks (not synchronously in the
 * effect body — `react-hooks/set-state-in-effect`). Initial `loading` is the
 * useState default; re-polls keep the previous ready view until the new
 * result lands (or a fault replaces it, fail-closed).
 */
export default function YourStakePanel({ refreshKey }: YourStakePanelProps) {
  const [load, setLoad] = useState<StakeViewLoad>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    invoke<StakingView>("get_staking_view")
      .then((view) => {
        if (cancelled) return;
        // A nullish fulfill is not a valid WI-RPC-1 view (wire is always an
        // object). Treat as fault so we never render legs off a non-value.
        if (view == null) {
          console.error(
            "get_staking_view: null/undefined response (not a valid wire view)",
          );
          setLoad({ kind: "fault" });
          return;
        }
        setLoad({ kind: "ready", view });
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        // Operator diagnostics: seal/version/open faults arrive as strings
        // from the Tauri boundary; keep them in the webview console so a
        // support session can see *why* the panel is fail-closed without
        // putting protocol detail into the user-facing copy (rule 81/82).
        console.error("get_staking_view failed:", err);
        setLoad({ kind: "fault" });
      });
    return () => {
      cancelled = true;
    };
  }, [refreshKey]);

  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-2">
        <Lock className="h-4 w-4 text-gold-400" />
        <h2 className="text-sm font-semibold text-purple-200">Your stake</h2>
      </div>

      {load.kind === "fault" && (
        <p className="text-xs text-red-300">
          Staking state could not be read. This is a read fault, not an empty
          stake — retrying on the next refresh.
        </p>
      )}

      {load.kind === "loading" && (
        <p className="text-xs text-purple-300">Reading staking state…</p>
      )}

      {load.kind === "ready" && <StakeViewBody view={load.view} />}
    </div>
  );
}

function StakeViewBody({ view }: { view: StakingView }) {
  return (
    <>
      <div className="grid grid-cols-3 gap-4">
        <Leg
          amount={view.bonded_principal_confirmed}
          label="Bonded (confirmed)"
          title="Bond principal locked under confirmed live bonds."
        />
        <Leg
          amount={view.bonded_principal_pending}
          label="Bonded (pending)"
          title="Bond principal committed by sealed posts not yet confirmed on chain."
        />
        <Leg
          amount={view.rewards_received_unspent}
          label="Rewards (unspent)"
          title="Emission rewards received to your staker persona and still unspent."
        />
      </div>

      {view.staked_outputs.length === 0 ? (
        <p className="text-xs text-purple-300">
          No staked outputs yet. Rewards and persona funding will appear here
          once observed on chain.
        </p>
      ) : (
        <div>
          <p className="mb-2 text-xs font-semibold text-purple-200">
            Staked outputs ({view.staked_outputs.length})
          </p>
          <ul className="space-y-1">
            {view.staked_outputs.map((o) => (
              <li
                key={o.gindex}
                className="flex items-center justify-between rounded-lg bg-purple-900/40 px-3 py-2 text-xs"
              >
                <span className="text-purple-200">Slot {o.p_slot}</span>
                <span className="font-semibold text-white">
                  {formatSklCompact(o.amount)} SKL
                </span>
                <span
                  className="text-purple-300"
                  title="Block height at which this output becomes spendable."
                >
                  unlocks at {o.unlock_height.toLocaleString()}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {view.recovery_pending_reopen && (
        <p
          className="rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-200"
          role="status"
        >
          Staking recovered an earlier attempt when this wallet opened. Close
          and reopen the wallet to finish — staking actions will not work
          until you do.
        </p>
      )}

      <p className="text-[10px] text-purple-300/80">
        {view.pscan_synced_height === null
          ? "Persona scan has not sealed a frontier yet."
          : `Persona scan synced to block ${view.pscan_synced_height.toLocaleString()}.`}
      </p>
    </>
  );
}

function Leg({
  amount,
  label,
  title,
}: {
  amount: number;
  label: string;
  title: string;
}) {
  return (
    <div className="flex flex-col items-center gap-1">
      <span className="text-sm font-bold text-white">
        {formatSklCompact(amount)}
      </span>
      <span className="text-center text-[10px] text-purple-300" title={title}>
        {label}
      </span>
    </div>
  );
}
