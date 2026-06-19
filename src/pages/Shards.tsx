import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Boxes, ImageIcon, RefreshCw } from "lucide-react";
import type {
  ShardRenderResponse,
  ShardSummary,
} from "../types/shards";

const RENDER_SIZE = 160;

/**
 * Shards page — lists every shard the wallet can see and renders its
 * deterministic identity visual. Backed by the `ShardSource` abstraction in
 * `shekyl-shard-source`: today that is the embedded regime fixtures; when
 * Stage 5 archival lands the same UI renders real archived shards with no
 * change here.
 */
export default function Shards() {
  const [shards, setShards] = useState<ShardSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<ShardSummary[]>("list_shards");
      setShards(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Boxes className="h-5 w-5 text-gold-400" />
            <h1 className="text-xl font-bold text-purple-100">Shards</h1>
          </div>
          <p className="mt-1 text-sm text-purple-300">
            Each shard is a frozen segment of chain history with its own
            deterministic identity visual. Stakers archive shards as their
            useful work once archival staking ships.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary shrink-0 gap-2 px-3 py-2 text-xs"
          onClick={() => void load()}
          disabled={loading}
        >
          <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </div>

      <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-3 text-xs text-amber-100/90">
        <div className="flex items-start gap-2">
          <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-300" />
          <p>
            Pre-archival preview. These shards are representative chain regimes,
            not live wallet data — production renders from real archived
            segments once Stage 5 archival lands. Visuals are read-only chain
            renderings; they are never traded or transferred.
          </p>
        </div>
      </div>

      {error && (
        <div className="flex items-center justify-between gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          <span>{error}</span>
          <button
            type="button"
            className="btn btn-secondary px-2 py-1 text-xs"
            onClick={() => void load()}
          >
            Retry
          </button>
        </div>
      )}

      {loading && shards.length === 0 ? (
        <div className="flex items-center justify-center py-16 text-purple-300">
          <RefreshCw className="mr-2 h-5 w-5 animate-spin" /> Loading shards…
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {shards.map((shard) => (
            <ShardCard key={shard.aggregate.shard_id} shard={shard} />
          ))}
        </div>
      )}
    </div>
  );
}

function ShardCard({ shard }: { shard: ShardSummary }) {
  const { aggregate, label } = shard;
  const [png, setPng] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<ShardRenderResponse>("get_shard_render", {
      handle: {
        shard_id: aggregate.shard_id,
        shard_hash: aggregate.shard_hash,
        hash_override: null,
        size: RENDER_SIZE,
      },
    })
      .then((res) => {
        if (!cancelled) setPng(res.png_base64);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [aggregate.shard_id, aggregate.shard_hash]);

  const claimed = aggregate.stake_events_claimed;
  const created = aggregate.stake_events_created;

  return (
    <div className="card space-y-3">
      <div className="flex items-center gap-3">
        <div className="relative flex h-20 w-20 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-purple-600/40 bg-purple-950/60">
          {png ? (
            <img
              src={`data:image/png;base64,${png}`}
              alt={label}
              className="h-full w-full object-cover"
              width={RENDER_SIZE}
              height={RENDER_SIZE}
            />
          ) : error ? (
            <AlertTriangle className="h-5 w-5 text-red-300" />
          ) : (
            <ImageIcon className="h-5 w-5 animate-pulse text-purple-400" />
          )}
        </div>
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-purple-100">{label}</p>
          <p className="text-xs text-purple-400">Shard #{aggregate.shard_id}</p>
          <p className="mt-0.5 inline-block rounded-full border border-purple-600/40 bg-purple-900/40 px-2 py-0.5 text-[10px] uppercase tracking-wide text-purple-300">
            {aggregate.dominant_regime}
          </p>
        </div>
      </div>

      <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-xs">
        <Stat label="Blocks" value={aggregate.block_count.toLocaleString()} />
        <Stat label="Txs" value={aggregate.tx_count.toLocaleString()} />
        <Stat label="Outputs" value={aggregate.output_count.toLocaleString()} />
        <Stat
          label="Stakes"
          value={`${created.toLocaleString()} / ${claimed.toLocaleString()}`}
          hint="created / claimed"
        />
      </dl>

      <div className="rounded-md border border-purple-700/40 bg-purple-950/40 px-2 py-1.5">
        <p className="text-[10px] uppercase tracking-wide text-purple-400">
          Tier distribution (short / med / long)
        </p>
        <p className="font-mono text-xs text-purple-200">
          {aggregate.tier_distribution.join(" / ")}
        </p>
      </div>

      <p className="break-all font-mono text-[10px] text-purple-500" title={aggregate.shard_hash}>
        {aggregate.shard_hash.slice(0, 24)}…
      </p>
    </div>
  );
}

function Stat({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-purple-400" title={hint}>
        {label}
      </dt>
      <dd className="font-medium text-purple-100">{value}</dd>
    </div>
  );
}
