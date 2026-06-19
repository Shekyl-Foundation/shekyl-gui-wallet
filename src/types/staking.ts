/**
 * One live staked output's full lifecycle state.
 * Mirrors `shekyl_scanner::StakeView` (atomic-unit fields are integers;
 * `tx_hash` is lowercase hex).
 */
export interface StakeView {
  tx_hash: string;
  global_output_index: number;
  amount: number;
  tier: number;
  stake_height: number;
  stake_lock_until: number;
  blocks_until_mature: number;
  matured: boolean;
  pending_unstake: boolean;
  accrued_claimable: number;
  estimated_yield_to_maturity: number;
}

/** Response of the `unstake` command (`shekyl_scanner`-adjacent wire shape). */
export interface UnstakeResponse {
  tx_hash_list: string[];
  amount: number;
}
