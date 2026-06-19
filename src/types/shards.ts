import type { CandidateRecipe } from "./shardPreview";

/**
 * Public chain properties that drive a shard's visual semantics.
 * Mirrors `shekyl_shard_visual::ShardAggregate` (the f64 fields are
 * display-only aesthetic scalars; `shard_hash` is lowercase hex).
 */
export interface ShardAggregate {
  shard_id: number;
  shard_hash: string;
  block_count: number;
  tx_count: number;
  output_count: number;
  coinbase_output_count: number;
  time_range_seconds: number;
  coinbase_ratio: number;
  value_log_mean: number;
  value_log_variance: number;
  stake_events_created: number;
  stake_events_claimed: number;
  tier_distribution: [number, number, number];
  dominant_regime: string;
}

/** One entry in the shard list (`shekyl_shard_source::ShardSummary`). */
export interface ShardSummary {
  label: string;
  aggregate: ShardAggregate;
}

/** A render request for a single shard (`shekyl_shard_source::ShardRenderHandle`). */
export interface ShardRenderHandle {
  shard_id: number;
  shard_hash: string;
  hash_override?: string | null;
  size: number;
}

/** Render result for a single shard (`shard_visual::ShardRenderResponse`). */
export interface ShardRenderResponse {
  png_base64: string;
  recipe: CandidateRecipe;
  cache_key: string;
  shard_id: number;
}
