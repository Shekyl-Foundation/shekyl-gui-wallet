/**
 * Wire types for the send flow — the wallet contract's names, not the GUI's.
 *
 * Every atomic amount here is a decimal string (`AtomicUnitsString` in the
 * contract): a JS `number` cannot hold all `u64` values, and a fee or amount
 * rounded at the Tauri edge would defeat the one invariant this flow exists
 * for — the fee the user confirms is the fee that ships. Format with
 * `formatSkl`, parse with `parseSkl`, never through `Number`.
 */

export type AtomicUnitsString = string;

/** `get_default_fee_priority`: tier quotes for the canonical 2-in/2-out shape. */
export interface FeeTierQuote {
  economy: AtomicUnitsString;
  standard: AtomicUnitsString;
  high: AtomicUnitsString;
  tree_depth: number;
}

/** The fee tiers, lowercase, one spelling each (IG-Q52). */
export type FeePriorityTier = "economy" | "standard" | "high";

/** The tier a send uses when the user picks none. */
export const DEFAULT_TIER: FeePriorityTier = "standard";

/** `build_pending_tx`: the reservation the user is asked to confirm. */
export interface BuiltPendingTx {
  pending_tx_id: string;
  /** The exact fee of this transaction. */
  fee: AtomicUnitsString;
  /** Pass back as `seenGen` on submit. */
  content_gen: number;
}

export type SubmitVerdict = "ACCEPTED" | "ALREADY_IN_POOL" | "ALREADY_IN_CHAIN";

/** `submit_pending_tx` success. */
export interface SubmitResult {
  tx_hash: string;
  verdict: SubmitVerdict;
  /** False when the send went out and stands but the wallet could not save the node's answer. */
  verdict_saved: boolean;
  confirmed_height: number | null;
}

/** The typed rejection every send command uses. `code` is the contract's name. */
export interface SendError {
  code: string;
  message: string;
  /**
   * Where the reservation stands, as the engine says:
   * - `held`: still the page's; nothing was sent. Keep it, to confirm again
   *   once the cause is fixed or to cancel. Forgetting it would leave its
   *   inputs locked with nothing to release them.
   * - `in-flight`: the daemon may have it. Let go, and never discard it.
   * - `gone`: released, or none was touched.
   */
  reservation: "held" | "in-flight" | "gone";
}

export function isSendError(e: unknown): e is SendError {
  return (
    typeof e === "object" &&
    e !== null &&
    typeof (e as SendError).code === "string" &&
    typeof (e as SendError).reservation === "string"
  );
}

export function sendErrorMessage(e: unknown): string {
  return isSendError(e) ? e.message : String(e);
}

/**
 * The chain moved under a reservation the user was reviewing. Shared by
 * Send and stake funding: both confirm one built transaction.
 */
export const CONTENT_CHANGED_NOTICE =
  "The chain moved while you were reviewing and the transaction's fee or change " +
  "would have differed. It has been rebuilt — please check the figures again before confirming.";

/** A submit that may be on the network. Never discarded: that could spend twice. */
export const IN_FLIGHT_ADVICE = "Refresh your balance and check Transactions before trying again.";

/** A submit that sent nothing; the reservation is still the page's. */
export const HELD_ADVICE = "Nothing was sent. Confirm to try again, or cancel to release the funds.";

/** Cancel could not release the reservation. It is still this page's. */
export const RELEASE_FAILED_ADVICE =
  "The reservation could not be released; your funds stay reserved until it is. Try Cancel again.";
