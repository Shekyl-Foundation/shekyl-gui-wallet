import { useEffect, useState, useCallback, useRef, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ChainHealth,
  PqcStatus,
  SecurityStatus,
  WalletStatus,
} from "../types/daemon";
import { DaemonContext } from "./daemonState";

// Poll cadence. The fast interval is used until the first successful connect
// so users aren't waiting 30s to see the Dashboard. The slow interval takes
// over afterwards to avoid hammering the daemon.
const FAST_POLL_MS = 2_000;
const SLOW_POLL_MS = 30_000;

export function DaemonProvider({ children }: { children: ReactNode }) {
  const [health, setHealth] = useState<ChainHealth | null>(null);
  const [pqc, setPqc] = useState<PqcStatus | null>(null);
  const [security, setSecurity] = useState<SecurityStatus | null>(null);
  const [status, setStatus] = useState<WalletStatus | null>(null);
  const [connected, setConnected] = useState(false);
  const [hasEverConnected, setHasEverConnected] = useState(false);
  const [waitingSeconds, setWaitingSeconds] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // `useState` with a lazy initializer captures the start time exactly once,
  // without violating the react-hooks purity rule (calling `Date.now()`
  // directly during render is flagged).
  const [startedAt] = useState(() => Date.now());
  const hasEverConnectedRef = useRef(false);

  const fetchData = useCallback(async () => {
    // Cheap probe first — in production this Tauri command never throws; it
    // reports `connected: false` on daemon unreachable. In tests `invoke`
    // is mocked and may resolve to undefined, so we defend against that too.
    let nextStatus: WalletStatus | null = null;
    try {
      const raw = await invoke<WalletStatus | undefined>("get_wallet_status");
      nextStatus = raw ?? null;
    } catch (e) {
      // Only happens if the Tauri bridge itself is broken, not on daemon
      // outages. Treat as hard error.
      setError(String(e));
      setConnected(false);
      setLoading(false);
      return;
    }

    setStatus(nextStatus);
    setConnected(nextStatus?.connected ?? false);

    if (!nextStatus?.connected) {
      // Don't spam the daemon with the heavier RPCs while it's offline.
      setLoading(false);
      return;
    }

    // Daemon is up — fetch the richer chain/security data. Each call is
    // best-effort and kept independent so a single transient failure
    // doesn't flip us back to the connecting screen.
    const [healthRes, pqcRes, securityRes] = await Promise.allSettled([
      invoke<ChainHealth>("get_chain_health"),
      invoke<PqcStatus>("get_pqc_status"),
      invoke<SecurityStatus>("get_security_status"),
    ]);

    if (healthRes.status === "fulfilled") setHealth(healthRes.value);
    if (pqcRes.status === "fulfilled") setPqc(pqcRes.value);
    if (securityRes.status === "fulfilled") setSecurity(securityRes.value);

    if (!hasEverConnectedRef.current) {
      hasEverConnectedRef.current = true;
      setHasEverConnected(true);
    }
    setError(null);
    setLoading(false);
  }, []);

  // Variable-cadence poll: fast until first connect, slow afterwards.
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const tick = async () => {
      if (cancelled) return;
      await fetchData();
      if (cancelled) return;
      const delay = hasEverConnectedRef.current ? SLOW_POLL_MS : FAST_POLL_MS;
      timer = setTimeout(tick, delay);
    };

    void tick();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [fetchData]);

  // Drive the `waitingSeconds` counter while we're waiting for the first
  // connect. Used by the connecting screen to show "waited Xs" and reveal
  // secondary copy after a threshold.
  useEffect(() => {
    if (hasEverConnected) return;
    const id = setInterval(() => {
      setWaitingSeconds(Math.floor((Date.now() - startedAt) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, [hasEverConnected, startedAt]);

  return (
    <DaemonContext.Provider
      value={{
        health,
        pqc,
        security,
        status,
        connected,
        hasEverConnected,
        waitingSeconds,
        loading,
        error,
        refresh: fetchData,
      }}
    >
      {children}
    </DaemonContext.Provider>
  );
}
