import { AlertTriangle, History, ShieldCheck } from "lucide-react";

interface ProvenanceEntry {
  fingerprint: string;
  timestamp: number;
  source: string;
}

interface AddressProvenanceProps {
  current: ProvenanceEntry;
  history: ProvenanceEntry[];
  fingerprintChanged: boolean;
}

export default function AddressProvenance({
  current,
  history,
  fingerprintChanged,
}: AddressProvenanceProps) {
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <History className="h-4 w-4 text-purple-400" />
          <h3 className="text-sm font-medium text-purple-200">
            Address Provenance
          </h3>
        </div>
        {fingerprintChanged && (
          <span className="flex items-center gap-1 rounded-full bg-red-500/10 px-2.5 py-0.5 text-xs font-medium text-red-400">
            <AlertTriangle className="h-3 w-3" />
            Changed
          </span>
        )}
      </div>

      {fingerprintChanged && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/5 p-3 text-sm text-red-300">
          <p className="font-medium">Address fingerprint has changed!</p>
          <p className="mt-1 text-xs text-red-400">
            The multisig address data differs from what was previously recorded.
            Verify with all group members before sending funds.
          </p>
        </div>
      )}

      <div className="rounded-lg border border-purple-700/30 bg-purple-900/20 p-3 space-y-1.5">
        <div className="flex items-center gap-2">
          <ShieldCheck className="h-3.5 w-3.5 text-green-400" />
          <span className="text-xs font-medium text-purple-200">Current</span>
        </div>
        <code className="block font-mono text-xs text-gold-300">
          {current.fingerprint.match(/.{1,4}/g)?.join(" ")}
        </code>
        <span className="text-xs text-purple-500">
          {current.source} · {new Date(current.timestamp * 1000).toLocaleDateString()}
        </span>
      </div>

      {history.length > 0 && (
        <div className="space-y-2">
          <span className="text-xs font-medium text-purple-400">
            Previous ({history.length})
          </span>
          {history.map((entry, i) => (
            <div
              key={i}
              className="rounded-lg border border-purple-800/30 bg-purple-950/20 p-2.5 space-y-1"
            >
              <code className="block font-mono text-xs text-purple-400">
                {entry.fingerprint.match(/.{1,4}/g)?.join(" ")}
              </code>
              <span className="text-xs text-purple-600">
                {entry.source} · {new Date(entry.timestamp * 1000).toLocaleDateString()}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
