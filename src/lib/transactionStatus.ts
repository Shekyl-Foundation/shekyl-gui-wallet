import type { TransferState, UnspendableReason } from "../types/transfers";

/** How each contract `Transfer.state` reads on the Transactions page (rule 82: no arm collapsed). */
const STATE_META: Record<TransferState, { label: string; className: string; title?: string }> = {
  CONFIRMED: { label: "Confirmed", className: "text-purple-400" },
  PENDING: { label: "Pending", className: "text-amber-400" },
  FAILED: {
    label: "Failed",
    className: "text-red-400",
    title: "The network refused this send. It was never mined — you can try again.",
  },
  DROPPED: {
    label: "Dropped",
    className: "text-orange-400",
    title: "The wallet stopped waiting for this send. Your funds are spendable again.",
  },
  ABANDONED: {
    label: "Abandoned",
    className: "text-orange-300",
    title:
      "You told the wallet to stop tracking this send. If the network confirms it later, it will show as Confirmed.",
  },
  SPENT: { label: "Spent", className: "text-purple-500" },
  UNSPENDABLE: {
    label: "Unspendable",
    className: "text-purple-500",
    title: "Received on chain, but this wallet can never spend it. It is counted in no balance.",
  },
};

/** Which half of a received-but-unspendable output failed, in ordinary language. */
const UNSPENDABLE_TITLE: Record<UnspendableReason, string> = {
  PQC_LEAF_MISMATCH:
    "Received on chain, but it was not created for this wallet, so it can never be spent. It is counted in no balance.",
  PQC_LEAF_ENTRY_ABSENT:
    "Received on chain, but the transaction is missing what a spend needs, so this wallet can never spend it. It is counted in no balance.",
};

export function statusLabel(state: TransferState): string {
  return STATE_META[state].label;
}

export function statusClass(state: TransferState): string {
  return STATE_META[state].className;
}

/** `reason` selects the unspendable sentence. Other states ignore it. */
export function statusTitle(state: TransferState, reason?: UnspendableReason): string | undefined {
  if (state === "UNSPENDABLE" && reason !== undefined) {
    return UNSPENDABLE_TITLE[reason];
  }
  return STATE_META[state].title;
}
