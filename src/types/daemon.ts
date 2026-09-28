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
  /** Atomic units, decimal string (`AtomicUnitsString`). */
  total_burned: string;
  staker_pool_balance: string;
  staker_emission_share_effective: number;
  emission_era: string;
  last_block_reward: string;
  last_block_timestamp: number;
  last_block_hash: string;
  last_block_size: number;
  total_staked: string;
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

/**
 * The contract's `GetBalanceResult` (`get_balance`), projected once in
 * engine-core for this wallet and wallet-rpc alike. Atomic amounts are decimal
 * strings; format with `formatSkl`. `staked` and `claimable_rewards` are
 * **absent, never `"0"`,** when the wallet's staking state could not be read:
 * the liquid fields stay authoritative while the staking figures degrade, and
 * the card must render "unavailable", not "nothing staked".
 */
export interface Balance {
  /** The one-glance figure; the engine assigns it and `unlocked` from the same ledger figure today. */
  liquid: string;
  /** Spendable right now. */
  unlocked: string;
  /** Committed to a send awaiting confirmation: counted, never spendable. */
  pending: string;
  /** Received but never spendable by this wallet; counted nowhere else. */
  unspendable: string;
  /** Bond principal under confirmed and in-flight bonds. */
  staked?: string;
  /** Emission rewards received and still unspent. Absent exactly when `staked` is. */
  claimable_rewards?: string;
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
  | { status: "ready"; spendable: string }
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
  /** Atomic units, decimal string. */
  block_reward: string;
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
 * Amounts are atomic-unit decimal strings.
 */
export interface StakedOutputView {
  gindex: number;
  /** Atomic units, decimal string. */
  amount: string;
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
  /** The three legs are atomic-unit decimal strings, never summed here. */
  bonded_principal_confirmed: string;
  bonded_principal_pending: string;
  rewards_received_unspent: string;
  staked_outputs: StakedOutputView[];
  pscan_synced_height: number | null;
  /**
   * A staked slot was adopted this session and cannot be acted on until the
   * wallet is reopened. Shown, never hidden: a wallet that displays
   * staker-hood it cannot use is the failure this flag exists to prevent.
   */
  recovery_pending_reopen: boolean;
}

