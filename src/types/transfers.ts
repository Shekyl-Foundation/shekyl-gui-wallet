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

/**
 * Which payment request an incoming transfer arrived against.
 * Each arm carries only the fields the contract defines for that kind.
 */
export type ReceiveAttribution =
  | { kind: "UNATTRIBUTED" }
  | { kind: "MATCHED"; request_id: string }
  | { kind: "MANUAL_MATCH"; request_id: string }
  | { kind: "LABEL_UNKNOWN"; echoed_label_hash: string }
  | { kind: "DISPUTED"; dispute_reason: string };

/** `GetTransfersParams.attribution`: a `ReceiveAttribution` kind without its payload. */
export type ReceiveAttributionKind = ReceiveAttribution["kind"];

interface TransferShared {
  /** Unique per wallet: `{tx_hash}:{output_index}` incoming, bare `{tx_hash}` outgoing. */
  id: string;
  /** Not unique across rows: a send and its change output share one hash. */
  tx_hash: string;
  amount: string;
  fee: string;
  /** Inclusion height; absent exactly when the transaction is not on chain. */
  block_height?: number;
}

type IncomingSettlement =
  | { state: "UNSPENDABLE"; unspendable_reason: UnspendableReason }
  | { state: Exclude<TransferState, "UNSPENDABLE"> };

/** A receive. Attribution is always present; `UNATTRIBUTED` is a kind, not a missing field. */
export type IncomingTransfer = TransferShared & {
  direction: "INCOMING";
  attribution: ReceiveAttribution;
} & IncomingSettlement;

/** A send. No attribution: the contract omits the field rather than inventing `UNATTRIBUTED`. */
export type OutgoingTransfer = TransferShared & {
  direction: "OUTGOING";
  state: Exclude<TransferState, "UNSPENDABLE">;
};

/** A row the projection can emit. */
export type Transfer = IncomingTransfer | OutgoingTransfer;

/** `get_transfers` result. */
export interface Transfers {
  transfers: Transfer[];
}
