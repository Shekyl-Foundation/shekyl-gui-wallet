/**
 * The wallet contract's `get_transfers` vocabulary (`wallet_rpc.yaml`,
 * `Transfer`), limited to the fields this GUI projects. Atomic amounts are
 * decimal strings; format with `formatSkl`.
 */

export type TransferDirection = "INCOMING" | "OUTGOING";

/** `Transfer.state`. Every arm the projection emits has its own label. */
export type TransferState =
  | "PENDING"
  | "CONFIRMED"
  | "SPENT"
  | "UNSPENDABLE"
  | "FAILED"
  | "DROPPED"
  | "ABANDONED";

/** `Transfer.unspendable_reason`. Present exactly when `state` is `UNSPENDABLE`. */
export type UnspendableReason = "PQC_LEAF_MISMATCH" | "PQC_LEAF_ENTRY_ABSENT";

/** `ReceiveAttribution.kind`: which payment request an incoming transfer arrived against. */
export type ReceiveAttributionKind =
  | "UNATTRIBUTED"
  | "MATCHED"
  | "LABEL_UNKNOWN"
  | "MANUAL_MATCH"
  | "DISPUTED";

/** Present on INCOMING rows only. Bookkeeping facts; no cleartext labels. */
export interface ReceiveAttribution {
  kind: ReceiveAttributionKind;
  /** Decimal `rid` when `MATCHED` / `MANUAL_MATCH`. */
  request_id?: string;
  echoed_label_hash?: string;
  dispute_reason?: string;
}

/** The contract's `GetTransfersParams`, as the page sends them (all optional). */
export interface TransferFilter {
  direction?: TransferDirection;
  state?: TransferState;
  since_height?: number;
}

interface TransferCommon {
  /** Unique per wallet: `{tx_hash}:{output_index}` incoming, bare `{tx_hash}` outgoing. */
  id: string;
  /** Not unique across rows: a send and its change output share one hash. */
  tx_hash: string;
  amount: string;
  fee: string;
  /** Inclusion height; absent exactly when the transaction is not on chain. */
  block_height?: number;
  direction: TransferDirection;
  /** Present on INCOMING rows only. */
  attribution?: ReceiveAttribution;
}

/** A row the projection can emit. The reason exists only on `UNSPENDABLE`. */
export type Transfer =
  | (TransferCommon & {
      state: "UNSPENDABLE";
      unspendable_reason: UnspendableReason;
    })
  | (TransferCommon & {
      state: Exclude<TransferState, "UNSPENDABLE">;
    });

/** `get_transfers` result. */
export interface Transfers {
  transfers: Transfer[];
}
