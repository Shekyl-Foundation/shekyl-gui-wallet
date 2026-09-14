import { useCallback, useMemo, useState, type ReactNode } from "react";
import type { ShardCoverageRow } from "../types/shards";
import { MAX_HOLDINGS_SHARDS } from "../types/shards";
import { ShardPickerContext } from "./shardPickerState";

export function ShardPickerProvider({ children }: { children: ReactNode }) {
  const [selected, setSelected] = useState<Map<number, number>>(
    () => new Map(),
  );

  const toggle = useCallback((shardId: number, expectedProfitAtomic: number) => {
    let accepted = true;
    setSelected((prev) => {
      const next = new Map(prev);
      if (next.has(shardId)) {
        next.delete(shardId);
        return next;
      }
      if (next.size >= MAX_HOLDINGS_SHARDS) {
        accepted = false;
        return prev;
      }
      next.set(shardId, expectedProfitAtomic);
      return next;
    });
    return accepted;
  }, []);

  const registerCoverage = useCallback((rows: readonly ShardCoverageRow[]) => {
    setSelected((prev) => {
      if (prev.size === 0) return prev;
      const next = new Map(prev);
      let changed = false;
      for (const row of rows) {
        if (next.has(row.shard_id)) {
          if (next.get(row.shard_id) !== row.expected_profit_atomic) {
            next.set(row.shard_id, row.expected_profit_atomic);
            changed = true;
          }
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
