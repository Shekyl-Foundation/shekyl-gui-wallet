import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send as SendIcon, AlertCircle } from "lucide-react";

export default function Send() {
  const [address, setAddress] = useState("");
  const [amount, setAmount] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  async function handleSend(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSending(true);
    try {
      await invoke("transfer", {
        address,
        amount: Math.round(parseFloat(amount) * 1e12),
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Send SKL</h1>

      <form onSubmit={handleSend} className="card space-y-5">
        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Recipient Address
          </label>
          <input
            type="text"
            className="input font-mono text-sm"
            placeholder="SKL1..."
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            required
          />
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium text-purple-200">
            Amount (SKL)
          </label>
          <input
            type="number"
            className="input"
            placeholder="0.0000"
            step="0.0001"
            min="0"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            required
          />
        </div>

        {error && (
          <div className="flex items-start gap-2 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={sending}
          className="btn btn-primary w-full"
        >
          <SendIcon className="h-4 w-4" />
          {sending ? "Sending..." : "Send"}
        </button>
      </form>
    </div>
  );
}
