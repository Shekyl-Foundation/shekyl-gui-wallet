/** Lifecycle arms from the Engine transfer projection (snake_case wire). */
export type TxStatus =
  | "confirmed"
  | "pending"
  | "failed"
  | "dropped"
  | "spent";

export type TxDirection = "in" | "out";

const STATUS_META: Record<
  TxStatus,
  { label: string; className: string; title?: string }
> = {
  confirmed: { label: "Confirmed", className: "text-purple-400" },
  pending: { label: "Pending", className: "text-amber-400" },
  failed: {
    label: "Failed",
    className: "text-red-400",
    title:
      "The network refused this send. It was never mined — you can try again.",
  },
  dropped: {
    label: "Dropped",
    className: "text-orange-400",
    title:
      "The wallet stopped waiting for this send. Your funds are spendable again.",
  },
  spent: { label: "Spent", className: "text-purple-500" },
};

export function statusLabel(status: string): string {
  return STATUS_META[status as TxStatus]?.label ?? status;
}

export function statusClass(status: string): string {
  return STATUS_META[status as TxStatus]?.className ?? "text-purple-400";
}

export function statusTitle(status: string): string | undefined {
  return STATUS_META[status as TxStatus]?.title;
}
