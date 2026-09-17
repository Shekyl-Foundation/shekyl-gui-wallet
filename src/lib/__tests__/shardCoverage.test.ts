import { describe, it, expect } from "vitest";
import { shuffleEqualScarcityBands } from "../shardCoverage";
import type { ShardCoverageRow } from "../../types/shards";

function row(
  shard_id: number,
  join_scarcity_micro: number,
  expected_profit_atomic: number | string = join_scarcity_micro,
): ShardCoverageRow {
  return {
    shard_id,
    bonded_count: 1,
    served_count: 1,
    freeze_height: 1,
    join_scarcity_micro,
    expected_profit_atomic: String(expected_profit_atomic),
  };
}

describe("shuffleEqualScarcityBands", () => {
  it("keeps unequal scarcity bands in daemon order", () => {
    const rows = [row(2, 90), row(1, 50), row(0, 10)];
    const out = shuffleEqualScarcityBands(rows, () => 0);
    expect(out.map((r) => r.shard_id)).toEqual([2, 1, 0]);
  });

  it("permutes only within an equal-scarcity band", () => {
    const rows = [row(3, 90), row(1, 50), row(2, 50), row(0, 10)];
    // random() === 0 swaps each k with the band start (Fisher–Yates).
    const out = shuffleEqualScarcityBands(rows, () => 0);
    expect(out.map((r) => r.shard_id)).toEqual([3, 2, 1, 0]);
  });

  it("does not merge equal-profit rows that still differ in scarcity", () => {
    const rows = [row(3, 900, 50), row(1, 100, 50), row(2, 10, 50)];
    const out = shuffleEqualScarcityBands(rows, () => 0);
    expect(out.map((r) => r.shard_id)).toEqual([3, 1, 2]);
  });

  it("does not shuffle the full list when every profit is zero", () => {
    const rows = [row(3, 900, 0), row(1, 100, 0), row(2, 10, 0)];
    const out = shuffleEqualScarcityBands(rows, () => 0);
    expect(out.map((r) => r.shard_id)).toEqual([3, 1, 2]);
  });
});
