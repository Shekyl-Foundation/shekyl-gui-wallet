/**
 * Where the wallet's own node stands (`daemon_status`, the app's command;
 * `src-tauri/src/daemon_manager.rs`).
 */
export type NodeState =
  | "idle"
  | "starting"
  | "running"
  | "stopping"
  | "waiting"
  | "found"
  | "service"
  | "refused"
  | "failed"
  | "unavailable";

/**
 * The node the wallet is pointed at (`daemon_connection_disclosures`,
 * `set_daemon_connection`). `warnings` is empty for a node on this machine
 * and never contains an assurance: the only thing the wallet can establish
 * about a node address is what it costs, never that it is safe.
 */
export interface DaemonConnection {
  /** The node address a person typed; `null` is this computer's node. */
  url: string | null;
  warnings: string[];
  /** The network the wallet is on. */
  network: string;
}

export interface NodeStatus {
  state: NodeState;
  /** The state in words, where there is something to say. */
  message: string | null;
  /** The network the node runs, or is to run. */
  network: string;
  /** Whether a force stop has a node to act on: one this wallet started. */
  can_force_stop: boolean;
  /**
   * Whether the wallet is waiting on a node it did not start, a wait a
   * person can end. Ending it kills nothing.
   */
  can_stop_waiting: boolean;
  /**
   * The miner could not be moved to the open wallet's address: it was
   * stopped, or may still pay the wallet open before. Shown in any state.
   */
  payout_fault: string | null;
  /**
   * A service node's view listener, while this computer's node is a
   * service with one: "Use the service's node" points the wallet at it.
   */
  service_view: string | null;
}
