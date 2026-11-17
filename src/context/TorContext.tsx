import { useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TorRead } from "../types/tor";
import { TorContext } from "./torState";

/** How often the node's Tor state is read. */
const TOR_POLL_MS = 5_000;

export function TorProvider({ children }: { children: ReactNode }) {
  const [read, setRead] = useState<TorRead | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<TorRead | undefined>("get_tor_status");
      setRead(next ?? null);
      setError(null);
    } catch (e) {
      // The node is not answering: a stale read would be a claim about a
      // node that may no longer be there.
      setRead(null);
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    // The first read is a timer's too: the effect sets no state itself.
    const first = setTimeout(refresh, 0);
    const timer = setInterval(refresh, TOR_POLL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [refresh]);

  return <TorContext.Provider value={{ read, error, refresh }}>{children}</TorContext.Provider>;
}
