import { useContext } from "react";
import { TorContext } from "./torState";

/**
 * The node's Tor read, one for the whole app: the header line, the send
 * review, the pending list and Settings all read this, so no two of them
 * can say different things.
 */
export function useTor() {
  return useContext(TorContext);
}
