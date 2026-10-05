import { ShieldCheck } from "lucide-react";
import { formatSkl, SKL_DECIMALS } from "../../lib/format";

interface ReviewCardProps {
  address: string;
  /** Atomic units, exact. */
  amount: bigint;
  /** Atomic units, exact — the fee of the reservation that will be submitted. */
  fee: bigint;
}

/**
 * What the user consents to: recipient, amount, the exact fee, and the total,
 * all rendered at full `SKL_DECIMALS` precision from BigInt so that two
 * reservations differing by one atomic unit never display alike.
 */
export default function ReviewCard({ address, amount, fee }: ReviewCardProps) {
  const exact = (atomic: bigint) => `${formatSkl(atomic, SKL_DECIMALS)} SKL`;
  return (
    <div
      className="space-y-2 rounded-lg border border-gold-500/40 bg-purple-800/40 p-3 text-sm"
      data-testid="review"
    >
      <div className="flex justify-between text-purple-200">
        <span>To</span>
        <span className="max-w-[60%] break-all text-right font-mono text-xs text-white">{address}</span>
      </div>
      <div className="flex justify-between text-purple-200">
        <span>Amount</span>
        <span className="font-mono text-white">{exact(amount)}</span>
      </div>
      <div className="flex justify-between text-purple-200">
        <span>Fee</span>
        <span className="font-mono text-gold-400" data-testid="exact-fee">
          {exact(fee)}
        </span>
      </div>
      <div className="flex justify-between border-t border-purple-600/40 pt-2 font-medium text-white">
        <span>Total</span>
        <span className="font-mono" data-testid="total">
          {exact(amount + fee)}
        </span>
      </div>
      <p className="flex items-center gap-1 text-[10px] text-emerald-300">
        <ShieldCheck className="h-2.5 w-2.5" />
        Full-chain membership proof with post-quantum protection
      </p>
    </div>
  );
}
