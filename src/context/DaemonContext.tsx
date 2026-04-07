import { useEffect, useState, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ChainHealth, PqcStatus, SecurityStatus } from "../types/daemon";
import { DaemonContext } from "./daemonState";

const POLL_INTERVAL = 30_000;

export function DaemonProvider({ children }: { children: ReactNode }) {
  const [health, setHealth] = useState<ChainHealth | null>(null);
  const [pqc, setPqc] = useState<PqcStatus | null>(null);
  const [security, setSecurity] = useState<SecurityStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const [h, p, s] = await Promise.all([
        invoke<ChainHealth>("get_chain_health"),
        invoke<PqcStatus>("get_pqc_status"),
        invoke<SecurityStatus>("get_security_status").catch(() => null),
      ]);
      setHealth(h);
      setPqc(p);
      setSecurity(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const id = setInterval(fetchData, POLL_INTERVAL);
    return () => clearInterval(id);
  }, [fetchData]);

  return (
    <DaemonContext.Provider value={{ health, pqc, security, loading, error, refresh: fetchData }}>
      {children}
    </DaemonContext.Provider>
  );
}
