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
