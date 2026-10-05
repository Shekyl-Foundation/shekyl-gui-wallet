import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function messageOf(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  return e instanceof Error ? e.message : String(e);
}

interface NoteOut {
  tx_hash: string;
  note: string | null;
}

interface TransferRow {
  id: string;
  tx_hash: string;
  amount: string;
  state: string;
}

/** Note, abandon, and lookup by id. These sit beside the history list. */
export default function TxTools() {
  const [txHash, setTxHash] = useState("");
  const [note, setNote] = useState("");
  const [lookupId, setLookupId] = useState("");
  const [found, setFound] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const saveNote = async () => {
    setBusy(true);
    setError(null);
    try {
      const stored = await invoke<NoteOut>("set_tx_note", { txHash, note });
      setFound(stored.note ? `Note stored on ${stored.tx_hash}.` : `Note cleared on ${stored.tx_hash}.`);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const loadNote = async () => {
    setBusy(true);
    setError(null);
    try {
      const stored = await invoke<NoteOut>("get_tx_note", { txHash });
      setNote(stored.note ?? "");
      setFound(stored.note ? "Note loaded." : "No note stored for this transaction.");
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const abandon = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("abandon_tx", { txHash });
      setFound(
        "Send abandoned. Its funds stay locked until the network is confirmed to have dropped it.",
      );
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const lookup = async () => {
    setBusy(true);
    setError(null);
    try {
      const row = await invoke<TransferRow>("get_transfer_by_id", { id: lookupId });
      setFound(`${row.state}: ${row.amount} (id ${row.id})`);
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-3">
      <h2 className="text-sm font-semibold text-purple-200">A single transaction</h2>
      <input
        className="input"
        placeholder="Transaction id"
        value={txHash}
        onChange={(e) => setTxHash(e.target.value)}
      />
      <textarea
        className="input min-h-16"
        placeholder="Note"
        value={note}
        onChange={(e) => setNote(e.target.value)}
      />
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn btn-ghost" disabled={busy || txHash.length === 0} onClick={() => void loadNote()}>
          Load note
        </button>
        <button type="button" className="btn btn-primary" disabled={busy || txHash.length === 0} onClick={() => void saveNote()}>
          Save note
        </button>
        <button type="button" className="btn btn-ghost" disabled={busy || txHash.length === 0} onClick={() => void abandon()}>
          Abandon send
        </button>
      </div>
      <div className="flex gap-2">
        <input
          className="input"
          placeholder="Look up by id"
          value={lookupId}
          onChange={(e) => setLookupId(e.target.value)}
        />
        <button type="button" className="btn btn-ghost" disabled={busy || lookupId.length === 0} onClick={() => void lookup()}>
          Open
        </button>
      </div>
      {error && <p className="text-xs text-red-300">{error}</p>}
      {found && <p className="text-xs text-emerald-200">{found}</p>}
    </div>
  );
}
