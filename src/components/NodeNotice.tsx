import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Loader2, ShieldAlert } from "lucide-react";
import type { NodeStatus } from "../types/node";

/** How often the wallet's own node is asked where it stands. */
const NODE_STATUS_POLL_MS = 2000;

/**
 * Where the wallet's own node stands, whenever that is something a person
 * must know or can act on. The words are the backend's; this component adds
 * only the acts.
 *
 * - `found`: a node of this account already runs on the network. The wallet
 *   does not use it, and offers to stop it and start its own.
 * - `stopping`: a node is stopping; the wallet's own starts when it is gone.
 *   Force stop is offered throughout, for a node the wallet started, and is
 *   the only thing that kills one.
 * - `failed`: the node did not start, or stopped. Its own words, and a start.
 * - `refused`: something that is not this account's node answers at the
 *   node's local address. A security failure: said whole, with no act.
 * - `unavailable`: this build carries no node.
 *
 * Nothing is shown while the node is starting or running.
 */
export default function NodeNotice() {
  const [status, setStatus] = useState<NodeStatus | null>(null);
  const [acting, setActing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setStatus(await invoke<NodeStatus>("daemon_status"));
    } catch {
      // The command is the app's own; a failure here has nothing to say.
    }
  }, []);

  useEffect(() => {
    load();
    const timer = setInterval(load, NODE_STATUS_POLL_MS);
    return () => clearInterval(timer);
  }, [load]);

  // Each act names its command as a literal: the command-surface gate
  // reads the names from the source.
  async function act(run: () => Promise<unknown>) {
    setActing(true);
    setError(null);
    try {
      await run();
    } catch (e) {
      setError(String(e));
    } finally {
      setActing(false);
      load();
    }
  }
  const replaceFound = () => act(() => invoke("replace_found_node"));
  const startNode = () => act(() => invoke("start_node"));
  const forceStop = () => act(() => invoke("force_stop_node"));

  if (!status) return null;
  const { state, message } = status;
  if (state === "idle" || state === "starting" || state === "running") {
    return null;
  }

  const security = state === "refused";
  const Icon = security ? ShieldAlert : state === "stopping" ? Loader2 : AlertTriangle;

  return (
    <div
      role={security ? "alert" : "status"}
      className={`flex flex-wrap items-center gap-3 px-4 py-2 text-xs ${
        security ? "bg-red-500/25 text-red-100" : "bg-amber-500/15 text-amber-100"
      }`}
    >
      <Icon className={`h-4 w-4 shrink-0 ${state === "stopping" ? "animate-spin" : ""}`} />
      <span className="flex-1">
        {state === "stopping" ? "Stopping the node…" : message}
        {error && <span className="ml-2 text-red-200">{error}</span>}
      </span>
      {state === "found" && (
        <button
          className="rounded bg-amber-500/30 px-3 py-1 font-semibold hover:bg-amber-500/40 disabled:opacity-50"
          disabled={acting}
          onClick={replaceFound}
        >
          Stop it and start the wallet’s own
        </button>
      )}
      {state === "failed" && (
        <button
          className="rounded bg-amber-500/30 px-3 py-1 font-semibold hover:bg-amber-500/40 disabled:opacity-50"
          disabled={acting}
          onClick={startNode}
        >
          Start the node
        </button>
      )}
      {state === "stopping" && status.can_force_stop && (
        <button
          className="rounded bg-red-500/30 px-3 py-1 font-semibold text-red-100 hover:bg-red-500/40"
          onClick={forceStop}
        >
          Force stop
        </button>
      )}
    </div>
  );
}
