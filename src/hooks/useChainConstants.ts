import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ChainConstants } from "../types/daemon";

/**
 * Core's consensus constants the pages name (`get_chain_constants`). They
 * are this build's and need no node, so they are read once; `null` until the
 * read returns, or if the bridge itself fails.
 */
export function useChainConstants(): ChainConstants | null {
  const [constants, setConstants] = useState<ChainConstants | null>(null);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const c = await invoke<ChainConstants | undefined>("get_chain_constants");
        if (!cancelled) setConstants(c ?? null);
      } catch {
        if (!cancelled) setConstants(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);
  return constants;
}
