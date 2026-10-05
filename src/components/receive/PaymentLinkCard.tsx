import { QRCodeSVG } from "qrcode.react";
import { Copy, Check, X, AlertCircle } from "lucide-react";
import { useCopyFeedback } from "../../lib/useCopyFeedback";

interface PaymentLinkCardProps {
  uri: string;
  /** What the link is for, shown above the QR. */
  title: string;
  onDismiss: () => void;
}

/** A `shekyl:` payment link as QR and text, with copy. */
export default function PaymentLinkCard({ uri, title, onDismiss }: PaymentLinkCardProps) {
  const { state: copyState, copy } = useCopyFeedback();

  return (
    <div className="card space-y-3 text-center" data-testid="payment-link">
      <div className="flex items-center justify-between">
        <p className="text-sm font-semibold text-white">{title}</p>
        <button
          type="button"
          onClick={onDismiss}
          className="btn-ghost rounded-md p-1"
          aria-label="Dismiss payment link"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="mx-auto flex w-fit items-center justify-center rounded-xl bg-white p-3">
        <QRCodeSVG value={uri} size={192} level="M" marginSize={0} bgColor="#ffffff" fgColor="#1a1025" />
      </div>
      <div className="flex items-start gap-2 rounded-lg bg-purple-800 p-3">
        <code className="flex-1 break-all text-left text-[11px] text-gold-400">{uri}</code>
        <button
          type="button"
          onClick={() => copy(uri)}
          className="btn-ghost shrink-0 rounded-md p-1.5"
          title={copyState === "failed" ? "Copy failed — select the link and copy it by hand" : "Copy payment link"}
          aria-label="Copy payment link"
        >
          {copyState === "copied" ? (
            <Check className="h-4 w-4 text-emerald-400" />
          ) : copyState === "failed" ? (
            <AlertCircle className="h-4 w-4 text-red-400" />
          ) : (
            <Copy className="h-4 w-4" />
          )}
        </button>
      </div>
      {copyState === "failed" && (
        <p className="text-[10px] text-red-300" role="alert">
          Copy failed. Select the link above and copy it by hand.
        </p>
      )}
      <p className="text-[10px] text-purple-400">
        Send this link to whoever is paying you. It carries the amount and the label. Pasting it
        fills in their address and amount. That payment does not mark this request paid yet.
      </p>
    </div>
  );
}
