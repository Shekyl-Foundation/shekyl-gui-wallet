import { createContext } from "react";
import type {
  ChainHealth,
  PqcStatus,
  SecurityStatus,
  WalletStatus,
} from "../types/daemon";

export interface DaemonState {
  health: ChainHealth | null;
  pqc: PqcStatus | null;
  security: SecurityStatus | null;
  status: WalletStatus | null;
  /** Daemon answered the most recent probe. */
  connected: boolean;
  /** At least one probe has succeeded since this provider mounted. */
  hasEverConnected: boolean;
  /** Seconds since the provider started probing without a successful connect. */
  waitingSeconds: number;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

export const DaemonContext = createContext<DaemonState>({
  health: null,
  pqc: null,
  security: null,
  status: null,
  connected: false,
  hasEverConnected: false,
  waitingSeconds: 0,
  loading: true,
  error: null,
  refresh: () => {},
});
