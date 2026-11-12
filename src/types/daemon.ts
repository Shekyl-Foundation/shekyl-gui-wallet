/**
 * The chain as the node reports it (`get_chain_health`): the node's
 * `daemon_status` on the 4.0 wire, with the tip's header and curve tree.
 * Heights are ordinals. Ratios are millionths. Amounts are atomic-unit
 * decimal strings. A read the node did not answer is `null`, never zero.
 */
export interface ChainHealth {
  height: number;
  /** `null` when the node has no target. */
  target_height: number | null;
  hash: string;
  /** A 128-bit value, as a decimal string. */
  difficulty: string;
  tx_count: number;
  /** `null` while the node's pool is retired. */
  txpool_count: number | null;
  synchronized: boolean;
  /** Coins emitted so far. */
  emission: string;
  emission_multiplier: number;
  burn: number;
  burn_total: string;
  /** The stakers' effective share of emission. */
  stake_share: number;
  /** Seconds. */
  block_spacing: number;
  network: string;
  last_block: LastBlock | null;
  curve_tree: CurveTree | null;
}

export interface LastBlock {
  reward: string;
  /** Unix seconds. */
  time: number;
  hash: string;
  /** Bytes. */
  weight: number;
}

export interface CurveTree {
  root: string;
  layers: number;
  leaf_count: number;
}

export interface WalletStatus {
  connected: boolean;
  wallet_open: boolean;
  wallet_name: string | null;
  daemon_address: string | null;
  network: string;
  synced: boolean;
  /** The node's tip height; `null` while it cannot be reached. */
  height: number | null;
  /** `null` when the node has no target. */
  target_height: number | null;
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
  /** The one-glance figure (engine-core `project_balance`, shekyl-core `48d515145`, assigns it and `unlocked` alike). */
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

/**
 * The node's miner (`get_mining_status`, the node's `mine_status`). The
 * miner pays the node's own mining address, set when the node starts. The
 * posted job's figures are `null` while no job is posted, and `cause`
 * says why.
 */
export interface MiningStatus {
  active: boolean;
  /** Hashes per second over the node's last window; `null` before one. */
  hash_rate: number | null;
  threads: number;
  /** The address the miner pays; `null` when the node has none set. */
  address: string | null;
  /** Seconds. */
  block_spacing: number;
  height: number | null;
  /** Atomic units, decimal string. */
  block_reward: string | null;
  /** A 128-bit value, as a decimal string. */
  difficulty: string | null;
  cause: string | null;
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
  /** `null` when the node could not say. */
  anonymity_set_size: number | null;
  tree_depth: number | null;
  tree_root_short: string | null;
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
  /**
   * Staking actions still outstanding. Empty means none. `kind` is `stake`,
   * `claim`, `drain`, or `exit`; `state` is `pending`, `dispatched`,
   * `refused`, `expired`, or `held`. An unknown string is shown as itself.
   */
  in_flight: InFlightView[];
  /** The last ending of each kind of staking action. Empty means none. */
  outcomes: StakingOutcomeView[];
}

/** One outstanding staking action (`get_staking_view`, WR-Q1). */
export interface InFlightView {
  kind: string;
  state: string;
  /** Block height of the first send. Null when it was never sent. */
  dispatched_at_height: number | null;
  /** Sends so far, the first included. Zero when never sent. */
  attempts: number;
  /** For `refused` and `expired`, the block after which it can no longer be mined. */
  funds_return_at: number | null;
  /** User sentence, when this state has one. */
  reason: string | null;
}

/** How one kind of staking action last ended (`get_staking_view`, WR-Q2). */
export interface StakingOutcomeView {
  kind: string;
  outcome: string;
  /** User sentence. Never an internal name. */
  reason: string;
  at_height: number;
  /** `returned`, or `none_held` when nothing was set aside. */
  funds: string;
}

