import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TorRead } from "../types/tor";
import { TorContext } from "./torState";

/** How long after one read of the node's Tor state the next is asked for. */
const TOR_POLL_MS = 5_000;

export function TorProvider({ children }: { children: ReactNode }) {
  const [read, setRead] = useState<TorRead | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Each read is numbered; an answer older than the last one shown is
  // dropped, so a slow read never paints over a newer one (a refresh asked
  // for after a fix is taken, beside the poll). Not state: never drawn.
  const generation = useRef(0);

  const refresh = useCallback(async () => {
    const mine = ++generation.current;
    try {
      const next = await invoke<TorRead | undefined>("get_tor_status");
      if (mine !== generation.current) return;
      setRead(next ?? null);
      setError(null);
    } catch (e) {
      if (mine !== generation.current) return;
      // The node is not answering: a stale read would be a claim about a
      // node that may no longer be there.
      setRead(null);
      setError(String(e));
    }
  }, []);

  // The next read is asked for once the one before it has finished, never
  // on a fixed beat: a read slower than the beat would otherwise be
  // numbered out by the next one every time, and nothing would be shown.
  useEffect(() => {
    let closed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await refresh();
      if (!closed) timer = setTimeout(poll, TOR_POLL_MS);
    };
    timer = setTimeout(poll, 0);
    return () => {
      closed = true;
      clearTimeout(timer);
    };
  }, [refresh]);

  return <TorContext.Provider value={{ read, error, refresh }}>{children}</TorContext.Provider>;
}
