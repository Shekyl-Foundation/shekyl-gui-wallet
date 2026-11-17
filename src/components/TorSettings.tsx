import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTor } from "../context/useTor";
import type { OfferedFix, TorFix } from "../types/tor";

interface TorSettingsProps {
  /** The fix in force, from the wallet's settings; `null` is the bundled Tor. */
  inForce: TorFix | null;
  /** Called after a fix is taken or put back, whether or not the node started. */
  onChanged: () => void;
}

/** What the wallet's settings hold, said plainly, while a fix is in force. */
function inForceWords(fix: TorFix): string {
  switch (fix.fix) {
    case "restore-managed":
      return `You named a Tor program for this node: ${fix.tor_file}`;
    case "operator":
      return `You set this node to use a Tor you run yourself, at ${fix.tor_socks}.`;
    case "clearnet-on":
      return "You turned clearnet on for this node. Its Tor is still the bundled one.";
    case "clearnet-only":
      return "You turned Tor off for this node. It syncs and sends over the open network.";
  }
}

/** What the act for each offered fix is called. The consequence beside it is core's sentence. */
const ACT: Record<OfferedFix["keyword"], string> = {
  "restore-managed": "Choose a Tor program…",
  operator: "Use my own Tor",
  "clearnet-on": "Turn clearnet on",
  "clearnet-only": "Turn Tor off",
};

/**
 * Settings → Network: the node's Tor state in core's words, the fixes its
 * read offers, and the fix in force (`docs/design/GUI_SYNC_LANE.md` GS-3,
 * GS-F7).
 *
 * The fixes are the person's acts: exactly the ones in the read, in its
 * order, each beside core's sentence for it, none pre-selected. Taking one
 * is kept in the wallet's settings and the node is started again with it.
 * The wallet does not judge a value; the node's refusal is shown in its own
 * words.
 *
 * Once Tor is off or an operator's, the read offers no fix, and a node
 * that will not start gives no read. So the fix in force is shown from the
 * wallet's settings, with the way back, whatever the node is doing.
 */
export default function TorSettings({ inForce, onChanged }: TorSettingsProps) {
  const { read, error: readError, refresh } = useTor();
  const [torSocks, setTorSocks] = useState("");
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function setFix(fix: TorFix | null) {
    setWorking(true);
    setError(null);
    try {
      await invoke("set_tor_fix", { fix });
    } catch (e) {
      // The setting is kept even when the node then refuses its start: the
      // words say why, and the way back stays below.
      setError(String(e));
    } finally {
      setWorking(false);
      onChanged();
      refresh();
    }
  }

  async function take(keyword: OfferedFix["keyword"]) {
    switch (keyword) {
      case "restore-managed": {
        const chosen = await openDialog({ multiple: false, directory: false });
        if (typeof chosen === "string") await setFix({ fix: "restore-managed", tor_file: chosen });
        return;
      }
      case "operator":
        return setFix({ fix: "operator", tor_socks: torSocks });
      case "clearnet-on":
        return setFix({ fix: "clearnet-on" });
      case "clearnet-only":
        return setFix({ fix: "clearnet-only" });
    }
  }

  const text = read ? read.text : readError;
  const fixes = read?.fixes ?? [];
  if (!text && !inForce && !error) return null;

  return (
    <div className="space-y-2">
      <h3 className="text-xs font-semibold text-purple-200">Tor</h3>
      {text && (
        <p data-testid="tor-read" className="whitespace-pre-line text-xs text-purple-300">
          {text}
        </p>
      )}

      {inForce && (
        <div
          data-testid="tor-fix-in-force"
          className="flex flex-wrap items-center justify-between gap-2 rounded border border-purple-600/40 p-2 text-xs text-purple-200"
        >
          <span className="break-all">{inForceWords(inForce)}</span>
          <button
            type="button"
            className="btn btn-ghost text-xs"
            disabled={working}
            onClick={() => setFix(null)}
          >
            {inForce.fix === "operator" ? "Use the bundled Tor again" : "Go back to the bundled Tor"}
          </button>
        </div>
      )}

      {fixes.length > 0 && (
        <ul className="space-y-2" data-testid="tor-fixes">
          {fixes.map((fix) => (
            <li
              key={fix.keyword}
              className="space-y-1 rounded border border-purple-600/40 p-2 text-xs text-purple-200"
            >
              <p>{fix.sentence}</p>
              {fix.keyword === "operator" && (
                <label className="block space-y-1">
                  <span className="text-purple-300">Your Tor’s SOCKS address</span>
                  <input
                    type="text"
                    className="input font-mono text-xs"
                    placeholder="127.0.0.1:9050"
                    value={torSocks}
                    onChange={(e) => setTorSocks(e.target.value)}
                  />
                </label>
              )}
              <button
                type="button"
                className="btn btn-ghost text-xs"
                disabled={working}
                onClick={() => take(fix.keyword)}
              >
                {ACT[fix.keyword]}
              </button>
            </li>
          ))}
        </ul>
      )}

      {working && <p className="text-xs text-purple-300">Starting the node again…</p>}
      {error && (
        <p role="alert" className="whitespace-pre-line text-xs text-red-300">
          {error}
        </p>
      )}
    </div>
  );
}
