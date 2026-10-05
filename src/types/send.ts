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
  default_priority: "STANDARD";
  economy_fee: AtomicUnitsString;
  standard_fee: AtomicUnitsString;
  priority_fee: AtomicUnitsString;
  tree_depth: number;
}

export type FeePriorityTier = "ECONOMY" | "STANDARD" | "PRIORITY";

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
  confirmed_height: number | null;
}

/** The typed rejection every send command uses. `code` is the contract's name. */
export interface SendError {
  code: string;
  message: string;
  /**
   * The engine still holds the reservation. The page must NOT discard it: an
   * ambiguous or still-pending submit may already be on the network.
   */
  reservation_retained: boolean;
}

export function isSendError(e: unknown): e is SendError {
  return (
    typeof e === "object" &&
    e !== null &&
    typeof (e as SendError).code === "string" &&
    typeof (e as SendError).reservation_retained === "boolean"
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

/** A submit the engine kept. Do not discard it; discarding could spend twice. */
export const RETAINED_ADVICE = "Refresh your balance and check Transactions before trying again.";

/** Cancel could not release the reservation. It is still this page's. */
export const RELEASE_FAILED_ADVICE =
  "The reservation could not be released; your funds stay reserved until it is. Try Cancel again.";
