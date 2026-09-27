import type { TransferState } from "../types/transfers";

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

export function statusLabel(state: string): string {
  return STATE_META[state as TransferState]?.label ?? state;
}

export function statusClass(state: string): string {
  return STATE_META[state as TransferState]?.className ?? "text-purple-400";
}

export function statusTitle(state: string): string | undefined {
  return STATE_META[state as TransferState]?.title;
}
