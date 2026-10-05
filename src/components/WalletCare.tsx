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

/** Same floor as creating a wallet. The command edge still screens length and NUL. */
const MIN_PASSWORD_LENGTH = 8;

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
  const [confirmPassword, setConfirmPassword] = useState("");
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
      setConfirmPassword("");
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
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="wallet-current-password">
            Current password
          </label>
          <input
            id="wallet-current-password"
            type="password"
            className="input"
            value={oldPassword}
            onChange={(e) => setOldPassword(e.target.value)}
            autoComplete="current-password"
          />
        </div>
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="wallet-new-password">
            New password
          </label>
          <input
            id="wallet-new-password"
            type="password"
            className="input"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            autoComplete="new-password"
          />
          <p className="text-[11px] text-purple-400">At least {MIN_PASSWORD_LENGTH} characters.</p>
        </div>
        <div className="space-y-1.5">
          <label className="text-xs font-medium text-purple-200" htmlFor="wallet-confirm-password">
            Confirm new password
          </label>
          <input
            id="wallet-confirm-password"
            type="password"
            className="input"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            autoComplete="new-password"
          />
          {confirmPassword.length > 0 && confirmPassword !== newPassword && (
            <p className="text-[11px] text-red-300">The new passwords do not match.</p>
          )}
        </div>
        <button
          type="button"
          className="btn btn-primary"
          disabled={
            busy ||
            oldPassword.length === 0 ||
            newPassword.length < MIN_PASSWORD_LENGTH ||
            newPassword !== confirmPassword
          }
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
