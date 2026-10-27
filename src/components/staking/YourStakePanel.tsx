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
   * The parent mounts the panel for any open wallet; whether the wallet
   * stakes comes from this panel's own read (`view.staking_enabled`).
   */
  refreshKey?: unknown;
}

/**
 * "Your stake" panel: bonded principal legs, unspent rewards, staked outputs,
 * and persona-scan frontier for a staker; outstanding staking work and how it
 * last ended for any open wallet. Owns its own fetch.
 *
 * A wallet that is not staking still reads its outstanding work and endings:
 * a first stake is in flight before `staking_enabled` is set, and a first
 * stake that never landed leaves its ending behind after the reopen that
 * clears the slot. Hiding the panel there would hide why the stake failed.
 * A wallet with nothing to show renders nothing, and so does the first read
 * until it lands.
 *
 * Fetch shape matches ShardCoverageGallery: the effect only schedules the external
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

  if (load.kind === "loading") return null;
  if (load.kind === "ready" && !hasAnythingToShow(load.view)) return null;

  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-2">
        <Lock className="h-4 w-4 text-gold-400" />
        <h2 className="text-sm font-semibold text-purple-200">Your stake</h2>
      </div>

      {load.kind === "fault" ? (
        <p className="text-xs text-red-300">
          Staking state could not be read. This is a read fault, not an empty
          stake — retrying on the next refresh.
        </p>
      ) : (
        <StakeViewBody view={load.view} />
      )}
    </div>
  );
}

/** A staker always has its holdings to show; anyone else, only its work. */
function hasAnythingToShow(view: StakingView): boolean {
  return (
    view.staking_enabled ||
    view.in_flight.length > 0 ||
    view.outcomes.length > 0 ||
    view.recovery_pending_reopen
  );
}

function StakeViewBody({ view }: { view: StakingView }) {
  return (
    <>
      {view.staking_enabled && <StakeHoldings view={view} />}
      <StakeWork view={view} />
      {view.staking_enabled && (
        <p className="text-[10px] text-purple-300/80">
          {view.pscan_synced_height === null
            ? "Persona scan has not sealed a frontier yet."
            : `Persona scan synced to block ${view.pscan_synced_height.toLocaleString()}.`}
        </p>
      )}
    </>
  );
}

/** The bonded legs, unspent rewards and staked outputs: a staker's only. */
function StakeHoldings({ view }: { view: StakingView }) {
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
    </>
  );
}

/**
 * Outstanding staking work and how each kind last ended, for any wallet.
 *
 * `funds_return_at` is the record's end date: past it the transaction can no
 * longer be mined, and the wallet settles the record once its scan reaches
 * it. The read does not say whether the record reserved any inputs (a
 * zero-fee reward claim reserves none), so the line states only the date and
 * never that funds are set aside.
 */
function StakeWork({ view }: { view: StakingView }) {
  return (
    <>
      {view.in_flight.length > 0 && (
        <div>
          <p className="mb-2 text-xs font-semibold text-purple-200">
            In progress ({view.in_flight.length})
          </p>
          <ul className="space-y-1">
            {view.in_flight.map((row, index) => (
              <li
                key={`${row.kind}-${row.state}-${index}`}
                className="rounded-lg bg-purple-900/40 px-3 py-2 text-xs"
              >
                <p className="text-purple-100">
                  {kindLabel(row.kind)} — {stateLabel(row.state)}
                </p>
                {row.reason && (
                  <p className="mt-1 text-purple-200">{row.reason}</p>
                )}
                {(row.attempts > 0 || row.funds_return_at != null) && (
                  <p className="mt-1 text-purple-300">
                    {row.attempts === 1 && "Sent once."}
                    {row.attempts > 1 && `Sent ${row.attempts} times.`}
                    {row.funds_return_at != null &&
                      `${row.attempts > 0 ? " " : ""}Decided when the wallet's scan reaches block ${row.funds_return_at.toLocaleString()}.`}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {view.outcomes.length > 0 && (
        <div>
          <p className="mb-2 text-xs font-semibold text-purple-200">
            Recently ended
          </p>
          <ul className="space-y-1">
            {view.outcomes.map((row, index) => (
              <li
                key={`${row.kind}-${row.at_height}-${index}`}
                className="rounded-lg bg-purple-900/40 px-3 py-2 text-xs"
              >
                <p className="text-purple-100">{kindLabel(row.kind)}</p>
                <p className="mt-1 text-purple-200">{row.reason}</p>
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
    </>
  );
}

/** Open strings: an unknown kind is shown as the wallet sent it. */
function kindLabel(kind: string): string {
  switch (kind) {
    case "stake":
      return "Stake";
    case "claim":
      return "Reward claim";
    case "drain":
      return "Return of staking funds";
    case "exit":
      return "Stake release";
    default:
      return kind;
  }
}

function stateLabel(state: string): string {
  switch (state) {
    case "pending":
      return "not sent yet";
    case "dispatched":
      return "sent, not confirmed";
    case "refused":
      return "a node refused it";
    case "expired":
      return "can no longer be mined";
    case "held":
      return "kept until it can be checked";
    default:
      return state;
  }
}

function Leg({
  amount,
  label,
  title,
}: {
  amount: string;
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
