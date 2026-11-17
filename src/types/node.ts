/**
 * Where the wallet's own node stands (`daemon_status`, the app's command;
 * `src-tauri/src/daemon_manager.rs`).
 */
export type NodeState =
  | "idle"
  | "starting"
  | "running"
  | "stopping"
  | "found"
  | "refused"
  | "failed"
  | "unavailable";

export interface NodeStatus {
  state: NodeState;
  /** The state in words, where there is something to say. */
  message: string | null;
  /** The network the node runs, or is to run. */
  network: string;
  /** Whether a force stop has a node to act on: one this wallet started. */
  can_force_stop: boolean;
}
