import type { ShardCoverageRow } from "../types/shards";

/**
 * Shuffle equal join-scarcity bands in place of a copy. Daemon order is
 * `join_scarcity_micro` descending then `shard_id` ascending; only equal
 * scarcity is a ranking tie (`ARCHIVAL_SHARD_SELECTION_LIST.md` SL-D4 / §9).
 * Expected profit is a floor of that scarcity, so grouping on profit would
 * merge unequally ranked rows — and when the estimate is unavailable every
 * profit is zero, which would shuffle the whole list.
 */
export function shuffleEqualScarcityBands<T extends ShardCoverageRow>(
  rows: readonly T[],
  random: () => number = Math.random,
): T[] {
  const out = [...rows];
  let i = 0;
  while (i < out.length) {
    let j = i + 1;
    while (
      j < out.length &&
      out[j].join_scarcity_micro === out[i].join_scarcity_micro
    ) {
      j += 1;
    }
    for (let k = j - 1; k > i; k -= 1) {
      const r = i + Math.floor(random() * (k - i + 1));
      const tmp = out[k];
      out[k] = out[r];
      out[r] = tmp;
    }
    i = j;
  }
  return out;
}
