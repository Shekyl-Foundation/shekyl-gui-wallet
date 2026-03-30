import { createContext } from "react";
import type { ChainHealth, PqcStatus } from "../types/daemon";

export interface DaemonState {
  health: ChainHealth | null;
  pqc: PqcStatus | null;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

export const DaemonContext = createContext<DaemonState>({
  health: null,
  pqc: null,
  loading: true,
  error: null,
  refresh: () => {},
});
