import { useCallback, useMemo, useState, type ReactNode } from "react";
import {
  OpenAttemptContext,
  type OpenAttempt,
  type OpenAttemptStore,
} from "./openAttempt";

/**
 * Holds the latest open refusal for the unlock phase. Mount it around that
 * phase's routes, not inside Unlock: navigating to create unmounts the page
 * and would otherwise drop the refusal.
 */
export function OpenAttemptProvider({ children }: { children: ReactNode }) {
  const [attempt, setAttempt] = useState<OpenAttempt | null>(null);
  const recordAttempt = useCallback((next: OpenAttempt) => {
    setAttempt(next);
  }, []);
  const clearAttempt = useCallback(() => {
    setAttempt(null);
  }, []);
  const value = useMemo<OpenAttemptStore>(
    () => ({ attempt, recordAttempt, clearAttempt }),
    [attempt, recordAttempt, clearAttempt],
  );
  return (
    <OpenAttemptContext.Provider value={value}>
      {children}
    </OpenAttemptContext.Provider>
  );
}
