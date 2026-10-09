import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Clock, ImageIcon, Loader2, WifiOff } from "lucide-react";
import { describeError } from "../../lib/errors";
import { formatSklCompact } from "../../lib/format";
import {
  SHARD_STILL_OPEN,
  SHARD_UNAVAILABLE,
  SHARD_VIEW_NOT_OFFERED,
  type ShardCoverageRow,
  type ShardViewRender,
} from "../../types/shards";

const RENDER_SIZE = 160;

/**
 * What the frame shows. The view comes from the wallet's daemon after a
 * real fetch from a holder, so every outcome short of a picture is a state
 * the user can read (rule 82): the fetch is in flight, the shard is still
 * open, no holder served it this time, this daemon does not offer views,
 * or the wallet faulted with its own sentence. None is an empty frame.
 */
export type ShardFrame =
  | { kind: "idle" }
  | { kind: "fetching" }
  | { kind: "rendered"; png: string }
  | { kind: "still_open" }
  | { kind: "unavailable"; message: string }
  | { kind: "not_offered"; message: string }
  | { kind: "fault"; message: string };

function frameFromRefusal(err: unknown): ShardFrame {
  const code =
    typeof err === "object" && err !== null
      ? (err as { code?: unknown }).code
      : undefined;
  const message = describeError(err);
  switch (code) {
    case SHARD_STILL_OPEN:
      return { kind: "still_open" };
    case SHARD_UNAVAILABLE:
      return { kind: "unavailable", message };
    case SHARD_VIEW_NOT_OFFERED:
      return { kind: "not_offered", message };
    default:
      return { kind: "fault", message };
  }
}

/**
 * One gallery card. The view fetch is lazy (visible or selected) so listing
 * coverage never asks the daemon to fetch every shard on page mount.
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
  const [frame, setFrame] = useState<ShardFrame>({ kind: "idle" });
  const [attempt, setAttempt] = useState(0);

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
    setFrame({ kind: "fetching" });
    invoke<ShardViewRender>("get_shard_view", {
      shardId: row.shard_id,
      size: RENDER_SIZE,
    })
      .then((res) => {
        if (cancelled) {
          return;
        }
        if (res.view.shard_id !== row.shard_id) {
          setFrame({
            kind: "fault",
            message: "The wallet answered for a different archive.",
          });
          return;
        }
        setFrame({ kind: "rendered", png: res.png_base64 });
      })
      .catch((e) => {
        if (!cancelled) {
          setFrame(frameFromRefusal(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [shouldFetch, row.shard_id, attempt]);

  const retry = (e: React.MouseEvent) => {
    e.stopPropagation();
    setAttempt((n) => n + 1);
  };

  const profitLabel = profitAvailable
    ? `${formatSklCompact(row.expected_profit_atomic)} SKL / epoch`
    : "profit estimate unavailable";

  return (
    <div
      role="button"
      tabIndex={0}
      aria-pressed={selected}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onToggle();
        }
      }}
      className={`card w-full cursor-pointer space-y-3 text-left transition ${
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
          <ShardFrameView frame={frame} onRetry={retry} />
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
    </div>
  );
}

/** The frame's contents for each state; the sentence is the `title`. */
function ShardFrameView({
  frame,
  onRetry,
}: {
  frame: ShardFrame;
  onRetry: (e: React.MouseEvent) => void;
}) {
  switch (frame.kind) {
    case "idle":
      return <ImageIcon className="h-5 w-5 text-purple-400" />;
    case "fetching":
      return (
        <span title="Fetching this archive from a holder…" aria-label="Fetching">
          <Loader2 className="h-5 w-5 animate-spin text-purple-300" />
        </span>
      );
    case "rendered":
      return (
        <img
          src={`data:image/png;base64,${frame.png}`}
          alt=""
          className="h-full w-full object-cover"
          width={RENDER_SIZE}
          height={RENDER_SIZE}
        />
      );
    case "still_open":
      return (
        <span
          title="This archive is still being written; it can be drawn once it closes."
          aria-label="Still open"
        >
          <Clock className="h-5 w-5 text-purple-300" />
        </span>
      );
    case "unavailable":
      return (
        <button
          type="button"
          onClick={onRetry}
          title={`Could not be retrieved: ${frame.message} Click to try again.`}
          aria-label="Could not be retrieved; retry"
          className="flex h-full w-full items-center justify-center"
        >
          <WifiOff className="h-5 w-5 text-amber-300" />
        </button>
      );
    case "not_offered":
      return (
        <span title={frame.message} aria-label="Not offered by this daemon">
          <AlertTriangle className="h-5 w-5 text-purple-300" />
        </span>
      );
    case "fault":
      return (
        <span title={frame.message} aria-label="Could not draw">
          <AlertTriangle className="h-5 w-5 text-red-300" />
        </span>
      );
  }
}
