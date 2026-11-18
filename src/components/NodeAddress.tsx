import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DaemonConnection } from "../types/node";

interface Props {
  /** The connection in effect, as last read; `null` before the first read. */
  connection: DaemonConnection | null;
  /** The backend applied a change: the connection now in effect. */
  onApplied: (applied: DaemonConnection) => void;
}

/**
 * Which node the wallet reads: this computer's, the one the wallet starts,
 * or a node address a person typed. One editor, shown in Settings and
 * before a wallet is open, since a wallet cannot be made or opened against
 * a node that refuses it and Settings is reached only with one open.
 *
 * An address is sent when the field holds one; an empty field is this
 * computer's node. A typed node stays typed until "Use this computer's
 * node". The backend's refusal is shown in its own words: with a wallet
 * open, the node it uses does not change until the wallet is closed.
 */
export default function NodeAddress({ connection, onApplied }: Props) {
  // The field is the person's once they edit it; until then it shows the
  // address in effect, so a later read never writes over what they type.
  const [typed, setTyped] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const inEffect = connection?.url ?? "";
  const field = typed ?? inEffect;
  const typedNode = connection !== null && connection.url !== null;

  async function apply(url: string | undefined) {
    if (!connection) return;
    setSaving(true);
    setError(null);
    try {
      const applied = await invoke<DaemonConnection>("set_daemon_connection", {
        network: connection.network,
        ...(url === undefined ? {} : { url }),
      });
      setTyped(null);
      onApplied(applied);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }
  const save = () => apply(field.trim() === "" ? undefined : field.trim());
  const useThisComputer = () => apply(undefined);

  return (
    <div className="space-y-2">
      <div className="space-y-2">
        <label className="text-xs text-purple-300" htmlFor="node-address">
          Node address
        </label>
        <input
          id="node-address"
          type="text"
          className="input font-mono text-sm"
          placeholder="Empty: this computer’s node"
          value={field}
          onChange={(e) => setTyped(e.target.value)}
        />
        <p className="text-[11px] text-purple-400">
          {typedNode
            ? "A node address you typed: the wallet asks it over the network."
            : "This computer’s node, the one the wallet starts."}
        </p>
      </div>
      <button
        onClick={save}
        disabled={saving || !connection}
        className="btn btn-secondary w-full text-xs"
      >
        {saving ? "Connecting..." : "Save & Reconnect"}
      </button>
      {typedNode && (
        <button
          onClick={useThisComputer}
          disabled={saving}
          className="btn btn-ghost w-full text-xs"
        >
          Use this computer’s node
        </button>
      )}
      {error && (
        <p role="alert" className="text-xs text-amber-200">
          {error}
        </p>
      )}
      {(connection?.warnings ?? []).map((warning) => (
        <p
          key={warning}
          className="rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[11px] leading-relaxed text-amber-200"
          role="status"
        >
          {warning}
        </p>
      ))}
    </div>
  );
}
