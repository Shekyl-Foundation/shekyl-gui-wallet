import type { ShardCoverageRow } from "../types/shards";

/**
 * Shuffle equal join-profit bands in place of a copy. Daemon order is
 * join-scarcity desc then shard_id asc; expected profit is a floor of that
 * scarcity, so equal-profit neighbours are a contiguous band even when
 * scarcity still differs. Shuffle that band so the gallery does not always
 * present the same id first inside a tie.
 */
export function shuffleEqualProfitBands<T extends ShardCoverageRow>(
  rows: readonly T[],
  random: () => number = Math.random,
): T[] {
  const out = [...rows];
  let i = 0;
  while (i < out.length) {
    let j = i + 1;
    while (
      j < out.length &&
      out[j].expected_profit_atomic === out[i].expected_profit_atomic
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
