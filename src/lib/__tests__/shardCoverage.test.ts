import { describe, it, expect } from "vitest";
import { shuffleEqualProfitBands } from "../shardCoverage";
import type { ShardCoverageRow } from "../../types/shards";

function row(
  shard_id: number,
  expected_profit_atomic: number,
  join_scarcity_micro = expected_profit_atomic,
): ShardCoverageRow {
  return {
    shard_id,
    bonded_count: 1,
    served_count: 1,
    freeze_height: 1,
    join_scarcity_micro,
    expected_profit_atomic,
  };
}

describe("shuffleEqualProfitBands", () => {
  it("keeps unequal profit bands in daemon order", () => {
    const rows = [row(2, 90), row(1, 50), row(0, 10)];
    const out = shuffleEqualProfitBands(rows, () => 0);
    expect(out.map((r) => r.shard_id)).toEqual([2, 1, 0]);
  });

  it("permutes only within an equal-profit band", () => {
    const rows = [row(3, 90), row(1, 50), row(2, 50), row(0, 10)];
    // random() always 0.99 → Fisher-Yates always swaps with the high end.
    const out = shuffleEqualProfitBands(rows, () => 0.99);
    expect(out.map((r) => r.expected_profit_atomic)).toEqual([90, 50, 50, 10]);
    expect(new Set(out.slice(1, 3).map((r) => r.shard_id))).toEqual(
      new Set([1, 2]),
    );
    expect(out[0].shard_id).toBe(3);
    expect(out[3].shard_id).toBe(0);
  });
});
