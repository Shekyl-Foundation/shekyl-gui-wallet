import { Coins, Lock, TrendingUp, ShieldCheck, Construction, BookOpen } from "lucide-react";
import { useDaemon } from "../context/useDaemon";
import { formatSklCompact, formatPercent } from "../lib/format";
import EmissionGauge from "../components/EmissionGauge";
import ShardIdentityPreview from "../components/staking/ShardIdentityPreview";

/**
 * Honesty-mode Staking page (GUI-PR0).
 *
 * Claim-era tier lock / claim-rewards UX is retired with shekyl-core's
 * archival staking model. Personal stake / claim actions return after the
 * Engine backend lands (activate staker → fund persona → later drain).
 * See shekyl-core docs:
 *   - design/ARCHIVAL_STAKE_ACTIVATION_PLAN.md
 *   - design/PRINCIPAL_STAKE_LIFECYCLE.md
 *   - STAKER_OPERATOR_GUIDE.md
 */
export default function Staking() {
  const { health } = useDaemon();

  const stakeRatioPct = health ? (health.stake_ratio / 1_000_000) * 100 : 0;
  const emSharePct = health
    ? (health.staker_emission_share_effective / 1_000_000) * 100
    : 0;

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Staking</h1>

      {/* Coming-soon honesty banner */}
      <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 p-4">
        <div className="flex items-start gap-3">
          <Construction className="mt-0.5 h-5 w-5 shrink-0 text-amber-300" />
          <div>
            <p className="text-sm font-semibold text-amber-200">
              Archival staking is not available in this build
            </p>
            <p className="mt-1 text-xs leading-relaxed text-amber-100/85">
              Shekyl staking is archival participation: your wallet becomes a
              staker persona (<span className="font-mono">P</span>), funds
              collateral, and holds chain-history shards as useful work.
              The old tier-lock and claim-rewards flow has been retired
              protocol-side. Activation, funding, and reward recovery ship
              in follow-up wallet releases once the Engine backend is wired.
            </p>
          </div>
        </div>
      </div>

      {/* Model narrative */}
      <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
        <div className="flex items-start gap-3">
          <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-400" />
          <div>
            <p className="text-sm font-semibold text-emerald-300">
              What staking will mean
            </p>
            <ul className="mt-2 list-disc space-y-1.5 pl-4 text-xs leading-relaxed text-emerald-200/80">
              <li>
                <strong className="text-emerald-200">Activate</strong> — become
                an archival staker (password re-auth; bond post scheduled, not
                instant broadcast).
              </li>
              <li>
                <strong className="text-emerald-200">Fund</strong> — send
                principal to your active persona as ordinary private transfers
                (no minimum; structured cover is the wallet default).
              </li>
              <li>
                <strong className="text-emerald-200">Hold shards</strong> —
                archive chain segments as the useful work that earns rewards.
              </li>
              <li>
                <strong className="text-emerald-200">Recover</strong> — later
                unbond collateral (after cooldown) and drain rewards back to
                your principal wallet.
              </li>
            </ul>
            <p className="mt-2 text-xs leading-relaxed text-emerald-200/70">
              Desktop scope targets principal-side actions first (activate,
              fund, later drain). Full operator duties (onion service,
              challenge answering) are documented for node operators, not
              required for every wallet user.
            </p>
          </div>
        </div>
      </div>

      <ShardIdentityPreview />

      {/* Network staking gauges — chain stats only, not personal yield */}
      {health && (
        <div className="card">
          <div className="mb-4 flex items-center gap-2">
            <Coins className="h-4 w-4 text-gold-400" />
            <h2 className="text-sm font-semibold text-purple-200">
              Network stats
            </h2>
          </div>
          <p className="mb-4 text-xs text-purple-300">
            These figures are network-wide daemon metrics, not your personal
            stake or an estimated APY.
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

      {/* Operator guide pointer */}
      <div className="rounded-xl border border-purple-600/30 bg-purple-900/40 p-4">
        <div className="flex items-start gap-3">
          <BookOpen className="mt-0.5 h-5 w-5 shrink-0 text-purple-300" />
          <div>
            <p className="text-sm font-semibold text-purple-100">
              For future operators
            </p>
            <p className="mt-1 text-xs leading-relaxed text-purple-200/80">
              Collateral release takes a multi-epoch cooldown. Do not drop a
              shard expecting to fund another immediately, and do not batch
              multiple activations on a shared schedule — both weaken your
              privacy or strand capital. Full guidance lives in shekyl-core&apos;s{" "}
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
