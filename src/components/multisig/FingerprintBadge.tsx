import { useState } from "react";
import { Copy, CheckCircle2, ShieldCheck } from "lucide-react";

interface FingerprintBadgeProps {
  fingerprint: string;
  mRequired: number;
  nTotal: number;
  spendAuthVersion: number;
  label?: string;
}

export default function FingerprintBadge({
  fingerprint,
  mRequired,
  nTotal,
  spendAuthVersion,
  label = "Address Fingerprint",
}: FingerprintBadgeProps) {
  const [copied, setCopied] = useState(false);

  const grouped =
    fingerprint.match(/.{1,4}/g)?.join(" ") ?? fingerprint;

  function handleCopy() {
    navigator.clipboard.writeText(fingerprint);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="rounded-xl border border-purple-700/50 bg-purple-900/30 p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ShieldCheck className="h-4 w-4 text-gold-400" />
          <span className="text-sm font-medium text-purple-200">{label}</span>
        </div>
        <span className="rounded-full bg-purple-800/50 px-2.5 py-0.5 text-xs font-medium text-purple-300">
          {mRequired}-of-{nTotal} · v{spendAuthVersion}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <code className="flex-1 rounded-lg bg-purple-950/50 px-3 py-2 font-mono text-sm tracking-wider text-gold-300">
          {grouped}
        </code>
        <button
          onClick={handleCopy}
          className="rounded-lg p-2 text-purple-400 transition-colors hover:bg-purple-800/50 hover:text-white"
          title="Copy fingerprint"
        >
          {copied ? (
            <CheckCircle2 className="h-4 w-4 text-green-400" />
          ) : (
            <Copy className="h-4 w-4" />
          )}
        </button>
      </div>

      <p className="text-xs text-purple-500">
        Compare this fingerprint with all group members before transacting.
        Mismatch means different address data.
      </p>
    </div>
  );
}
