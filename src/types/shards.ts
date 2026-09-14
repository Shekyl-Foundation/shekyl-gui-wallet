import type { CandidateRecipe } from "./shardPreview";

/**
 * One row of `get_archival_shard_coverage` (`COMMAND_RPC_GET_ARCHIVAL_SHARD_COVERAGE`).
 * Ranking uses join-adjusted scarcity; the GUI shows `expected_profit_atomic`.
 */
export interface ShardCoverageRow {
  shard_id: number;
  bonded_count: number;
  served_count: number;
  freeze_height: number;
  join_scarcity_micro: number;
  expected_profit_atomic: number;
}

/** Full coverage list. `frozen_count === 0` is an honest empty, not a fault. */
export interface ShardCoverageList {
  as_of_height: number;
  leaf_count: number;
  frozen_count: number;
  settled_epoch: number;
  budget_atomic: number;
  sigma_work_milli: number;
  profit_estimate_available: boolean;
  shards: ShardCoverageRow[];
}

/** Render result for a single shard (`shard_visual::ShardRenderResponse`). */
export interface ShardRenderResponse {
  png_base64: string;
  recipe: CandidateRecipe;
  cache_key: string;
  shard_id: number;
}

/** Bond holdings cap (`ArchivalBondValue::kMaxHoldings`). */
export const MAX_HOLDINGS_SHARDS = 4096;
