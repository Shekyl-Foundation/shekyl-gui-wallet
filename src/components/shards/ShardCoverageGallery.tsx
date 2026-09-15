import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Boxes, RefreshCw } from "lucide-react";
import ShardCard from "./ShardCard";
import { useShardPicker } from "../../context/useShardPicker";
import { formatSklCompact } from "../../lib/format";
import { shuffleEqualScarcityBands } from "../../lib/shardCoverage";
import type { ShardCoverageList, ShardCoverageRow } from "../../types/shards";
import { GALLERY_PAGE_SIZE, MAX_HOLDINGS_SHARDS } from "../../types/shards";

type Load<T> =
  | { kind: "loading" }
  | { kind: "ready"; value: T }
  | { kind: "fault"; message: string };

/**
 * Operator gallery panel: fetch + fail-closed render for daemon coverage.
 * The Shards page composes this; selection stays in `ShardPickerProvider`.
 */
export default function ShardCoverageGallery() {
  const {
    selectedCount,
    expectedProfitSumAtomic,
    atCap,
    isSelected,
    toggle,
    registerCoverage,
  } = useShardPicker();
  const [load, setLoad] = useState<Load<ShardCoverageList>>({
    kind: "loading",
  });
  const [capNotice, setCapNotice] = useState(false);
  const [visibleCount, setVisibleCount] = useState(GALLERY_PAGE_SIZE);

  const fetchList = useCallback(async () => {
    setLoad({ kind: "loading" });
    try {
      const list = await invoke<ShardCoverageList>("list_shards");
      setLoad({ kind: "ready", value: list });
      setVisibleCount(GALLERY_PAGE_SIZE);
    } catch (e) {
      setLoad({ kind: "fault", message: String(e) });
    }
  }, []);

  useEffect(() => {
    void fetchList();
  }, [fetchList]);

  useEffect(() => {
    if (load.kind === "ready") {
      registerCoverage(load.value.shards);
    }
  }, [load, registerCoverage]);

  const displayRows = useMemo(() => {
    if (load.kind !== "ready") return [];
    return shuffleEqualScarcityBands(load.value.shards);
  }, [load]);

  const visibleRows = displayRows.slice(0, visibleCount);
  const remaining = displayRows.length - visibleRows.length;

  const onToggle = (row: ShardCoverageRow) => {
    const ok = toggle(row.shard_id, row.expected_profit_atomic);
    setCapNotice(!ok);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Boxes className="h-5 w-5 text-gold-400" />
            <h1 className="text-xl font-bold text-purple-100">Shards</h1>
          </div>
          <p className="mt-1 text-sm text-purple-300">
            Expected SKL per epoch is a ranking hint for the archives you
            pick — the network does not assign them, and the figure is not a
            payout. Click a card to toggle it for this session.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary shrink-0 gap-2 px-3 py-2 text-xs"
          onClick={() => void fetchList()}
          disabled={load.kind === "loading"}
        >
          <RefreshCw
            className={`h-3.5 w-3.5 ${load.kind === "loading" ? "animate-spin" : ""}`}
          />
          Refresh
        </button>
      </div>

      {selectedCount > 0 && (
        <p className="text-sm text-purple-200">
          {selectedCount} of {MAX_HOLDINGS_SHARDS} archives selected
          {load.kind === "ready" && load.value.profit_estimate_available
            ? ` · about ${formatSklCompact(expectedProfitSumAtomic)} SKL / epoch (hint, not a payout)`
            : ""}
        </p>
      )}

      {capNotice && atCap && (
        <p className="text-xs text-amber-200">
          A bond can hold at most {MAX_HOLDINGS_SHARDS} archives. Deselect
          one before adding another.
        </p>
      )}

      {load.kind === "fault" && (
        <div className="flex items-center justify-between gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          <span className="flex items-start gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            Could not load archive coverage. {load.message}
          </span>
          <button
            type="button"
            className="btn btn-secondary px-2 py-1 text-xs"
            onClick={() => void fetchList()}
          >
            Retry
          </button>
        </div>
      )}

      {load.kind === "loading" && (
        <div className="flex items-center justify-center py-16 text-purple-300">
          <RefreshCw className="mr-2 h-5 w-5 animate-spin" /> Loading
          coverage…
        </div>
      )}

      {load.kind === "ready" && load.value.frozen_count === 0 && (
        <p className="rounded-lg border border-purple-700/40 bg-purple-950/40 px-4 py-6 text-sm text-purple-200">
          No frozen archives yet. Coverage is empty until the chain has
          frozen segments; this is not a preview of sample data.
        </p>
      )}

      {load.kind === "ready" && load.value.frozen_count > 0 && (
        <>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {visibleRows.map((row) => (
              <ShardCard
                key={row.shard_id}
                row={row}
                selected={isSelected(row.shard_id)}
                profitAvailable={load.value.profit_estimate_available}
                onToggle={() => onToggle(row)}
              />
            ))}
          </div>
          {remaining > 0 && (
            <button
              type="button"
              className="btn btn-secondary w-full py-2 text-xs"
              onClick={() =>
                setVisibleCount((n) => n + GALLERY_PAGE_SIZE)
              }
            >
              Show more ({remaining} remaining)
            </button>
          )}
        </>
      )}

      {atCap && (
        <p className="text-xs text-purple-400">
          Selection is at the bond holdings cap ({MAX_HOLDINGS_SHARDS}).
        </p>
      )}
    </div>
  );
}
