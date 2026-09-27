/**
 * The wallet contract's `get_transfers` vocabulary (`docs/api/wallet_rpc.yaml`,
 * `Transfer`). Atomic amounts are decimal strings; format with `formatSkl`.
 */

export type TransferDirection = "INCOMING" | "OUTGOING";

/**
 * `Transfer.state`. `UNSPENDABLE` is in the contract's enum; the GUI's
 * projection does not emit it yet, but a row carrying it must still render.
 */
export type TransferState =
  | "PENDING"
  | "CONFIRMED"
  | "SPENT"
  | "UNSPENDABLE"
  | "FAILED"
  | "DROPPED"
  | "ABANDONED";

export interface Transfer {
  /** Unique per wallet: `{tx_hash}:{output_index}` incoming, bare `{tx_hash}` outgoing. */
  id: string;
  /** Not unique across rows: a send and its change output share one hash. */
  tx_hash: string;
  amount: string;
  fee: string;
  /** Inclusion height; absent exactly when the transaction is not on chain. */
  block_height?: number;
  direction: TransferDirection;
  state: TransferState;
  /** GUI-only display facts the contract does not carry. */
  timestamp: number;
  pqc_protected: boolean;
}

/** `get_transfers` result. */
export interface Transfers {
  transfers: Transfer[];
}
