import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function messageOf(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  return e instanceof Error ? e.message : String(e);
}

interface ScanOut {
  blocks_processed: number;
  transfers_detected: number;
  synced_height: number;
}

/** Password change, and a manual refresh or rescan beside background sync. */
export default function WalletCare() {
  const [oldPassword, setOldPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const changePassword = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      await invoke("change_password", { oldPassword, newPassword });
      setOldPassword("");
      setNewPassword("");
      setNote("Password changed.");
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const reportScan = (label: string, result: ScanOut) => {
    setNote(
      `${label} finished: ${result.blocks_processed} blocks, ${result.transfers_detected} transfers, height ${result.synced_height}.`,
    );
  };

  const refreshNow = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      reportScan("Refresh", await invoke<ScanOut>("refresh"));
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  const rescan = async () => {
    setError(null);
    setNote(null);
    setBusy(true);
    try {
      reportScan("Rescan", await invoke<ScanOut>("rescan_blockchain"));
    } catch (e) {
      setError(messageOf(e));
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
        <button
          type="button"
          className="btn btn-ghost"
          disabled={busy}
          onClick={() => void rescan()}
        >
          Rebuild history
        </button>
      </div>
      <p className="text-xs text-purple-300">
        Rebuild history re-reads the chain. Your notes, payment requests, and stake records stay.
      </p>
      {error && <p className="text-xs text-red-300">{error}</p>}
      {note && <p className="text-xs text-emerald-200">{note}</p>}
    </div>
  );
}
