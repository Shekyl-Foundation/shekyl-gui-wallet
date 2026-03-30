import { useContext } from "react";
import { DaemonContext } from "./daemonState";

export function useDaemon() {
  return useContext(DaemonContext);
}
