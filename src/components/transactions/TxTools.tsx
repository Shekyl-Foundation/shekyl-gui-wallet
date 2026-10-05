import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../../lib/errors";
import { formatSkl } from "../../lib/format";
import { readStatus } from "../../lib/transactionStatus";

interface NoteOut {
  tx_hash: string;
  note?: string;
}

interface TransferRow {
  id: string;
  tx_hash: string;
  amount: string;
  state: string;
}

/** How much of a pasted id to repeat in the abandon confirmation. */
const ABANDON_ID_SHOWN = 16;

/** Note, abandon, and lookup by id. The lookup id is the page's, so a history row can fill it. */
export default function TxTools({
  lookupId,
  onLookupId,
}: {
  lookupId: string;
  onLookupId: (id: string) => void;
}) {
  const [txHash, setTxHash] = useState("");
  const [note, setNote] = useState("");
  const [found, setFound] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmAbandon, setConfirmAbandon] = useState(false);

  const saveNote = async () => {
    setBusy(true);
    setError(null);
    try {
      const stored = await invoke<NoteOut>("set_tx_note", { txHash, note });
      setFound(stored.note ? `Note stored on ${stored.tx_hash}.` : `Note cleared on ${stored.tx_hash}.`);
    } catch (e) {
      setError(describeError(e));
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
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  const abandon = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("abandon_tx", { txHash });
      setConfirmAbandon(false);
      setFound(
        "Send abandoned. Its funds stay locked until the network is confirmed to have dropped it.",
      );
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  const lookup = async () => {
    setBusy(true);
    setError(null);
    try {
      const found = await invoke<{ transfer: TransferRow }>("get_transfer_by_id", { id: lookupId });
      setFound(
        `${readStatus(found.transfer.state)}: ${formatSkl(found.transfer.amount)} SKL (id ${found.transfer.id})`,
      );
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-3">
      <h2 className="text-sm font-semibold text-purple-200">A single transaction</h2>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="tx-note-hash">
          Transaction id
        </label>
        <input
          id="tx-note-hash"
          className="input"
          placeholder="64 hex characters"
          value={txHash}
          onChange={(e) => {
            setTxHash(e.target.value);
            setConfirmAbandon(false);
          }}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="tx-note-body">
          Note
        </label>
        <textarea
          id="tx-note-body"
          className="input min-h-16"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
      </div>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn btn-ghost" disabled={busy || txHash.length === 0} onClick={() => void loadNote()}>
          Load note
        </button>
        <button type="button" className="btn btn-primary" disabled={busy || txHash.length === 0} onClick={() => void saveNote()}>
          Save note
        </button>
        {!confirmAbandon ? (
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy || txHash.length === 0}
            onClick={() => setConfirmAbandon(true)}
          >
            Abandon send
          </button>
        ) : (
          <div className="space-y-2 text-xs text-amber-100">
            <p>
              Abandon stops tracking this send ({txHash.slice(0, ABANDON_ID_SHOWN)}
              {txHash.length > ABANDON_ID_SHOWN ? "…" : ""}). Its funds stay locked until the
              network is confirmed to have dropped it.
            </p>
            <div className="flex gap-2">
              <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void abandon()}>
                Abandon this send
              </button>
              <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirmAbandon(false)}>
                Keep it
              </button>
            </div>
          </div>
        )}
      </div>
      <div className="flex items-end gap-2">
        <div className="min-w-0 flex-1 space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="tx-lookup-id">
            Look up by id
          </label>
          <input
            id="tx-lookup-id"
            className="input"
            placeholder="Paste an id, or choose Look up on a row"
            value={lookupId}
            onChange={(e) => onLookupId(e.target.value)}
          />
        </div>
        <button type="button" className="btn btn-ghost" disabled={busy || lookupId.length === 0} onClick={() => void lookup()}>
          Open
        </button>
      </div>
      {error && <p className="text-xs text-red-300">{error}</p>}
      {found && <p className="text-xs text-emerald-200">{found}</p>}
    </div>
  );
}
