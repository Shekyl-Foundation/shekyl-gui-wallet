import { useState } from "react";
import { Radio, Plus, Trash2, AlertCircle, CheckCircle2 } from "lucide-react";

interface Relay {
  url: string;
  operatorId: string;
}

interface RelayConfigProps {
  relays: Relay[];
  onRelaysChange: (relays: Relay[]) => void;
  minOperators?: number;
}

export default function RelayConfig({
  relays,
  onRelaysChange,
  minOperators = 3,
}: RelayConfigProps) {
  const [newUrl, setNewUrl] = useState("");
  const [newOp, setNewOp] = useState("");

  const uniqueOps = new Set(relays.map((r) => r.operatorId));
  const diversityOk = uniqueOps.size >= minOperators;

  function addRelay() {
    if (!newUrl.trim() || !newOp.trim()) return;
    onRelaysChange([...relays, { url: newUrl.trim(), operatorId: newOp.trim() }]);
    setNewUrl("");
    setNewOp("");
  }

  function removeRelay(idx: number) {
    onRelaysChange(relays.filter((_, i) => i !== idx));
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Radio className="h-4 w-4 text-purple-400" />
          <h3 className="text-sm font-medium text-purple-200">Relay Configuration</h3>
        </div>
        {diversityOk ? (
          <span className="flex items-center gap-1 text-xs text-green-400">
            <CheckCircle2 className="h-3 w-3" />
            {uniqueOps.size} operators
          </span>
        ) : (
          <span className="flex items-center gap-1 text-xs text-yellow-400">
            <AlertCircle className="h-3 w-3" />
            {uniqueOps.size}/{minOperators} operators
          </span>
        )}
      </div>

      {!diversityOk && (
        <div className="rounded-lg border border-yellow-500/30 bg-yellow-500/5 p-2.5 text-xs text-yellow-300">
          Minimum {minOperators} distinct relay operators required for censorship
          resistance.
        </div>
      )}

      <div className="space-y-2">
        {relays.map((relay, i) => (
          <div
            key={i}
            className="flex items-center gap-2 rounded-lg border border-purple-700/30 bg-purple-900/20 p-2.5"
          >
            <div className="flex-1 min-w-0">
              <div className="truncate font-mono text-xs text-purple-200">{relay.url}</div>
              <div className="text-xs text-purple-500">Operator: {relay.operatorId}</div>
            </div>
            <button
              onClick={() => removeRelay(i)}
              className="shrink-0 rounded p-1 text-purple-500 hover:bg-purple-800/50 hover:text-red-400"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>

      <div className="flex gap-2">
        <input
          type="text"
          className="input flex-1 text-xs"
          placeholder="wss://relay.example.com"
          value={newUrl}
          onChange={(e) => setNewUrl(e.target.value)}
        />
        <input
          type="text"
          className="input w-32 text-xs"
          placeholder="Operator ID"
          value={newOp}
          onChange={(e) => setNewOp(e.target.value)}
        />
        <button
          onClick={addRelay}
          disabled={!newUrl.trim() || !newOp.trim()}
          className="btn btn-primary shrink-0 px-3 disabled:opacity-50"
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
