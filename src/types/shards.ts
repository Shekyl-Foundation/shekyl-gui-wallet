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
  budget_atomic: string;
  sigma_work_milli: number;
  profit_estimate_available: boolean;
  shards: ShardCoverageRow[];
}

/**
 * The wallet contract's `GetShardViewResult`: one closed shard's aggregate,
 * answered by the wallet's daemon after a real fetch from a holder
 * (shekyl-core `docs/design/SHARD_VIEW_FETCH.md`). Every field is a
 * deterministic function of the shard; `close_height` is the cache key's
 * second half — a view whose close height moved is a view across a reorg.
 */
export interface ShardView {
  shard_id: number;
  shard_hash: string;
  archival_len: number;
  block_count: number;
  tx_count: number;
  output_count: number;
  coinbase_output_count: number;
  time_range_seconds: number;
  close_height: number;
}

/** `get_shard_view` on this edge: the view plus the candidate.v1 render. */
export interface ShardViewRender {
  view: ShardView;
  png_base64: string;
  recipe: CandidateRecipe;
  cache_key: string;
}

/**
 * The contract codes `get_shard_view` refuses with, each a state a card
 * shows (rule 82). Anything else is a fault with the wallet's own sentence.
 */
export const SHARD_STILL_OPEN = "SHARD_STILL_OPEN";
export const SHARD_UNAVAILABLE = "SHARD_UNAVAILABLE";
export const SHARD_VIEW_NOT_OFFERED = "SHARD_VIEW_NOT_OFFERED";

/** Bond holdings cap (`ArchivalBondValue::kMaxHoldings`). */
export const MAX_HOLDINGS_SHARDS = 4096;

/**
 * Cards mounted at once on the operator gallery. Selection lives in session
 * state for the full coverage set; this only bounds DOM / observers.
 */
export const GALLERY_PAGE_SIZE = 36;
