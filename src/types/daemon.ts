export interface ChainHealth {
  height: number;
  target_height: number;
  top_block_hash: string;
  difficulty: number;
  tx_count: number;
  tx_pool_size: number;
  database_size: number;
  version: string;
  synchronized: boolean;
  already_generated_coins: string;
  release_multiplier: number;
  burn_pct: number;
  stake_ratio: number;
  total_burned: number;
  staker_pool_balance: number;
  staker_emission_share_effective: number;
  emission_era: string;
  last_block_reward: number;
  last_block_timestamp: number;
  last_block_hash: string;
  last_block_size: number;
  total_staked: number;
  tier_0_lock_blocks: number;
  tier_1_lock_blocks: number;
  tier_2_lock_blocks: number;
  network: string;
  curve_tree_root?: string;
  curve_tree_leaf_count?: number;
  curve_tree_depth?: number;
}

export interface WalletStatus {
  connected: boolean;
  wallet_open: boolean;
  wallet_name: string | null;
  daemon_address: string | null;
  network: string;
  synced: boolean;
  sync_height: number;
  daemon_height: number;
}

export interface Balance {
  total: number;
  unlocked: number;
  staked: number;
}

/**
 * Drainable-P read result (DS-PR-3 PR-B; `get_drain_balance`).
 *
 * Discriminated union mirroring the core two-armed split: `"ready"` carries the
 * anchored aggregate spendable scalar (atomic units); `"syncing"` is the
 * transient anchor arm — render a placeholder, never a zero. A non-transient
 * fault is not a variant here — the command rejects, and the caller's `.catch`
 * renders "—" (never a fabricated zero). "syncing" is shown only for the
 * transient arm, never conflated with a fault.
 */
export type DrainBalance =
  | { status: "ready"; spendable: number }
  | { status: "syncing"; detail: string };

export interface TierYield {
  tier: number;
  lock_blocks: number;
  lock_duration_hours: number;
  yield_multiplier: number;
  estimated_apy: number;
}

export interface MiningStatus {
  active: boolean;
  speed: number;
  threads_count: number;
  address: string;
  pow_algorithm: string;
  is_background_mining_enabled: boolean;
  block_target: number;
  block_reward: number;
  difficulty: number;
}

export interface PqcStatus {
  enabled: boolean;
  scheme: string;
  classical: string;
  post_quantum: string;
  tx_version: number;
  description: string;
}

export interface SecurityStatus {
  scheme: string;
  classical: string;
  post_quantum: string;
  tx_version: number;
  anonymity_set_size: number;
  tree_depth: number;
  tree_root_short: string;
  reference_block_window: number;
  proof_type: string;
  max_inputs: number;
  estimated_proof_size_kb: number;
  paths_precomputed: boolean;
}

export interface CurveTreeInfo {
  root: string;
  depth: number;
  leaf_count: number;
  height: number;
}

/**
 * One unspent staked (P-owned) funding output (`get_staking_view`).
 * Amounts are atomic units, display-only (see `DrainBalance` note).
 */
export interface StakedOutputView {
  gindex: number;
  amount: number;
  p_slot: number;
  unlock_height: number;
  confirmed: boolean;
}

/**
 * WI-RPC-1 staking read view (`get_staking_view`; GUI-PR3b).
 *
 * The three balance legs are distinct on purpose — confirmed bond principal,
 * pending (in-flight post) principal, and received-unspent rewards are never
 * summed into one figure. A read fault is not a variant here: the command
 * rejects and the caller renders a non-value, never "nothing staked" over a
 * bad read (rule 82).
 */
export interface StakingView {
  staking_enabled: boolean;
  bonded_principal_confirmed: number;
  bonded_principal_pending: number;
  rewards_received_unspent: number;
  staked_outputs: StakedOutputView[];
  pscan_synced_height: number | null;
}

export interface WalletProgress {
  event_type: string;
  current: number;
  total: number;
  detail: string | null;
}
