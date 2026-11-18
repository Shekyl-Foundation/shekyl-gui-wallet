import { createContext } from "react";
import type { TorRead } from "../types/tor";

export interface TorState {
  /** The node's Tor read; `null` before the first one, and when it failed. */
  read: TorRead | null;
  /** Why the last read failed, in the backend's words. */
  error: string | null;
  refresh: () => void;
}

export const TorContext = createContext<TorState>({
  read: null,
  error: null,
  refresh: () => {},
});
