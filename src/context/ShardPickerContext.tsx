import { useCallback, useMemo, useState, type ReactNode } from "react";
import type { ShardCoverageRow } from "../types/shards";
import { MAX_HOLDINGS_SHARDS } from "../types/shards";
import { atomicAmount } from "../lib/format";
import { ShardPickerContext } from "./shardPickerState";

export function ShardPickerProvider({ children }: { children: ReactNode }) {
  // Each selected shard's profit estimate, `null` where the node has none.
  const [selected, setSelected] = useState<Map<number, bigint | null>>(
    () => new Map(),
  );

  const toggle = useCallback(
    (shardId: number, expectedProfitAtomic: string | null) => {
      // Decide from the render that handled the click so the caller (cap
      // notice) matches what we enqueue. The updater still guards a burst
      // of clicks in the same tick.
      const removing = selected.has(shardId);
      const rejected = !removing && selected.size >= MAX_HOLDINGS_SHARDS;
      if (!rejected) {
        const profit =
          expectedProfitAtomic === null ? null : atomicAmount(expectedProfitAtomic);
        setSelected((prev) => {
          const next = new Map(prev);
          if (next.has(shardId)) {
            next.delete(shardId);
            return next;
          }
          if (next.size >= MAX_HOLDINGS_SHARDS) {
            return prev;
          }
          next.set(shardId, profit);
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
      const live = new Map<number, bigint | null>();
      for (const row of rows) {
        live.set(
          row.shard_id,
          row.expected_profit_atomic === null ? null : atomicAmount(row.expected_profit_atomic),
        );
      }
      const next = new Map<number, bigint | null>();
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
    let sum = 0n;
    for (const v of selected.values()) {
      if (v === null) return null;
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
