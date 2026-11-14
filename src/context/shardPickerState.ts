import { createContext } from "react";
import { MAX_HOLDINGS_SHARDS } from "../types/shards";
import type { ShardCoverageRow } from "../types/shards";

export interface ShardPickerState {
  selectedIds: ReadonlySet<number>;
  selectedCount: number;
  /** The selected shards' summed estimate; `null` when one has none. */
  expectedProfitSumAtomic: bigint | null;
  atCap: boolean;
  isSelected: (shardId: number) => boolean;
  toggle: (shardId: number, expectedProfitAtomic: string | null) => boolean;
  registerCoverage: (rows: readonly ShardCoverageRow[]) => void;
}

export const ShardPickerContext = createContext<ShardPickerState>({
  selectedIds: new Set(),
  selectedCount: 0,
  expectedProfitSumAtomic: 0n,
  atCap: false,
  isSelected: () => false,
  toggle: () => false,
  registerCoverage: () => {},
});

export { MAX_HOLDINGS_SHARDS };
