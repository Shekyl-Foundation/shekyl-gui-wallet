import { createContext } from "react";
import type { ChainHealth, PqcStatus, SecurityStatus } from "../types/daemon";

export interface DaemonState {
  health: ChainHealth | null;
  pqc: PqcStatus | null;
  security: SecurityStatus | null;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

export const DaemonContext = createContext<DaemonState>({
  health: null,
  pqc: null,
  security: null,
  loading: true,
  error: null,
  refresh: () => {},
});
