import { useCallback, useMemo, useState, type ReactNode } from "react";
import type { ShardCoverageRow } from "../types/shards";
import { MAX_HOLDINGS_SHARDS } from "../types/shards";
import { ShardPickerContext } from "./shardPickerState";

export function ShardPickerProvider({ children }: { children: ReactNode }) {
  const [selected, setSelected] = useState<Map<number, number>>(
    () => new Map(),
  );

  const toggle = useCallback(
    (shardId: number, expectedProfitAtomic: number) => {
      // Decide from the render that handled the click so the caller (cap
      // notice) matches what we enqueue. The updater still guards a burst
      // of clicks in the same tick.
      const removing = selected.has(shardId);
      const rejected = !removing && selected.size >= MAX_HOLDINGS_SHARDS;
      if (!rejected) {
        setSelected((prev) => {
          const next = new Map(prev);
          if (next.has(shardId)) {
            next.delete(shardId);
            return next;
          }
          if (next.size >= MAX_HOLDINGS_SHARDS) {
            return prev;
          }
          next.set(shardId, expectedProfitAtomic);
          return next;
        });
      }
      return !rejected;
    },
    [selected],
  );

  const registerCoverage = useCallback((rows: readonly ShardCoverageRow[]) => {
    setSelected((prev) => {
      if (prev.size === 0) return prev;
      const live = new Map<number, number>();
      for (const row of rows) {
        live.set(row.shard_id, row.expected_profit_atomic);
      }
      const next = new Map<number, number>();
      let changed = false;
      for (const [id, profit] of prev) {
        const fresh = live.get(id);
        if (fresh === undefined) {
          changed = true;
          continue;
        }
        next.set(id, fresh);
        if (fresh !== profit) {
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, []);

  const isSelected = useCallback(
    (shardId: number) => selected.has(shardId),
    [selected],
  );

  const expectedProfitSumAtomic = useMemo(() => {
    let sum = 0;
    for (const v of selected.values()) {
      sum += v;
    }
    return sum;
  }, [selected]);

  const value = useMemo(
    () => ({
      selectedIds: new Set(selected.keys()),
      selectedCount: selected.size,
      expectedProfitSumAtomic,
      atCap: selected.size >= MAX_HOLDINGS_SHARDS,
      isSelected,
      toggle,
      registerCoverage,
    }),
    [selected, expectedProfitSumAtomic, isSelected, toggle, registerCoverage],
  );

  return (
    <ShardPickerContext.Provider value={value}>
      {children}
    </ShardPickerContext.Provider>
  );
}
