import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../lib/errors";

interface RefreshOut {
  blocks_processed: number;
  transfers_detected: number;
  synced_height: number;
  reorg_fork_height?: number;
}

interface RescanOut {
  blocks_processed: number;
  transfers_detected: number;
  synced_height: number;
}

function scanSentence(
  label: string,
  result: { blocks_processed: number; transfers_detected: number; synced_height: number },
): string {
  return `${label} finished: ${result.blocks_processed} blocks, ${result.transfers_detected} transfers, height ${result.synced_height}.`;
}

/** Password change, and a manual refresh or rescan beside background sync. */
export default function WalletCare() {
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirmRescan, setConfirmRescan] = useState(false);

  const changePassword = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      await invoke("change_password", { oldPassword, newPassword });
      setOldPassword("");
      setNewPassword("");
      setNote("Password changed.");
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const refreshNow = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      const result = await invoke<RefreshOut>("refresh");
      const reorg = result.reorg_fork_height !== undefined
        ? " The chain reorganized, and history was rebuilt from the fork."
        : "";
      setNote(`${scanSentence("Refresh", result)}${reorg}`);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const rescan = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      const result = await invoke<RescanOut>("rescan_blockchain");
      setNote(scanSentence("Rescan", result));
      setConfirmRescan(false);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-4">
      <h2 className="text-sm font-semibold text-purple-200">Wallet</h2>
      <div className="space-y-2">
        <p className="text-xs text-purple-300">Change the password that opens this wallet.</p>
        <input
          type="password"
          className="input"
          placeholder="Current password"
          value={oldPassword}
          onChange={(e) => setOldPassword(e.target.value)}
          autoComplete="current-password"
        />
        <input
          type="password"
          className="input"
          placeholder="New password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          autoComplete="new-password"
        />
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy || oldPassword.length === 0 || newPassword.length === 0}
          onClick={() => void changePassword()}
        >
          Change password
        </button>
      </div>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => void refreshNow()}>
          Refresh now
        </button>
        {!confirmRescan ? (
          <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirmRescan(true)}>
            Rebuild history
          </button>
        ) : (
          <div className="space-y-2 text-xs text-amber-100">
            <p>
              Rebuild history re-reads the chain from the start. It can take a
              long time. Notes, payment requests, and stake records stay.
            </p>
            <div className="flex gap-2">
              <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void rescan()}>
                Rebuild now
              </button>
              <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirmRescan(false)}>
                Keep history
              </button>
            </div>
          </div>
        )}
      </div>
      {error && <p className="text-xs text-red-300">{error}</p>}
      {note && <p className="text-xs text-emerald-200">{note}</p>}
    </div>
  );
}
