import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, ImageIcon } from "lucide-react";
import { formatSklCompact } from "../../lib/format";
import type { ShardCoverageRow, ShardRenderResponse } from "../../types/shards";

const RENDER_SIZE = 160;

/**
 * One gallery card. PNG fetch is lazy (visible or selected) so listing
 * coverage never dials every shard on page mount.
 */
export default function ShardCard({
  row,
  selected,
  profitAvailable,
  onToggle,
}: {
  row: ShardCoverageRow;
  selected: boolean;
  profitAvailable: boolean;
  onToggle: () => void;
}) {
  const frameRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [png, setPng] = useState<string | null>(null);
  const [renderError, setRenderError] = useState<string | null>(null);

  useEffect(() => {
    const el = frameRef.current;
    if (!el || typeof IntersectionObserver === "undefined") {
      return;
    }
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true);
        }
      },
      { rootMargin: "80px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  const shouldFetch = visible || selected;

  useEffect(() => {
    if (!shouldFetch) {
      return;
    }
    let cancelled = false;
    invoke<ShardRenderResponse>("get_shard_render", {
      shardId: row.shard_id,
      size: RENDER_SIZE,
    })
      .then((res) => {
        if (!cancelled) {
          setPng(res.png_base64);
          setRenderError(null);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setRenderError(String(e));
          setPng(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [shouldFetch, row.shard_id]);

  const profitLabel = profitAvailable
    ? `${formatSklCompact(row.expected_profit_atomic)} SKL / epoch`
    : "profit estimate unavailable";

  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onToggle}
      className={`card w-full space-y-3 text-left transition ${
        selected
          ? "border-gold-400/60 ring-1 ring-gold-400/40"
          : "hover:border-purple-500/60"
      }`}
    >
      <div className="flex items-center gap-3">
        <div
          ref={frameRef}
          className="relative flex h-20 w-20 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-purple-600/40 bg-purple-950/60"
        >
          {png ? (
            <img
              src={`data:image/png;base64,${png}`}
              alt=""
              className="h-full w-full object-cover"
              width={RENDER_SIZE}
              height={RENDER_SIZE}
            />
          ) : renderError ? (
            <span title={renderError}>
              <AlertTriangle className="h-5 w-5 text-red-300" />
            </span>
          ) : (
            <ImageIcon className="h-5 w-5 text-purple-400" />
          )}
        </div>
        <div className="min-w-0">
          <p className="text-sm font-semibold text-purple-100">
            Archive #{row.shard_id}
          </p>
          <p className="mt-0.5 text-xs font-medium text-gold-300">
            {profitLabel}
          </p>
          {selected && (
            <p className="mt-1 text-[10px] uppercase tracking-wide text-gold-400">
              Selected
            </p>
          )}
        </div>
      </div>

      <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-xs">
        <div className="flex items-baseline justify-between gap-2">
          <dt className="text-purple-400">Bonded</dt>
          <dd className="font-medium text-purple-100">
            {row.bonded_count.toLocaleString()}
          </dd>
        </div>
        <div className="flex items-baseline justify-between gap-2">
          <dt className="text-purple-400">Served</dt>
          <dd className="font-medium text-purple-100">
            {row.served_count.toLocaleString()}
          </dd>
        </div>
      </dl>
    </button>
  );
}
