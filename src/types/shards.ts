import type { CandidateRecipe } from "./shardPreview";

/**
 * One row of `list_shards` (daemon `get_archival_shard_coverage`).
 * Ranking uses join-adjusted scarcity; the GUI shows `expected_profit_atomic`.
 * That field is a decimal string of atomic units so values above 2^53 stay
 * exact across the Tauri JSON edge.
 */
export interface ShardCoverageRow {
  shard_id: number;
  bonded_count: number;
  served_count: number;
  freeze_height: number;
  join_scarcity_micro: number;
  expected_profit_atomic: string;
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

/**
 * Cards mounted at once on the operator gallery. Selection lives in session
 * state for the full coverage set; this only bounds DOM / observers.
 */
export const GALLERY_PAGE_SIZE = 36;
