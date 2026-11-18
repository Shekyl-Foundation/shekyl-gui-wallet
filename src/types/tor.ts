/**
 * The node's Tor state for the page (`get_tor_status`, the app's command;
 * `src-tauri/src/tor_status.rs`).
 */

/** What happens to a transaction this wallet sends through the node now. */
export type Sends = "hidden" | "held" | "open-network" | "offline" | "unknown" | "withheld";

/** A fix the node's read offers, by core's keyword and in core's words. */
export interface OfferedFix {
  keyword: "restore-managed" | "operator" | "clearnet-on" | "clearnet-only";
  sentence: string;
}

export interface TorRead {
  /** Whether the node showed its Tor state. */
  shown: boolean;
  /** Core's rendering, verbatim; or that the state is withheld. */
  text: string;
  sends: Sends;
  /** The fixes the read offers, in its order. */
  fixes: OfferedFix[];
}

/**
 * The Tor fix in force, as the wallet's settings keep it
 * (`src-tauri/src/tor_fix.rs`). `null` is the node's defaults with the
 * bundled Tor.
 */
export type TorFix =
  | { fix: "restore-managed"; tor_file: string }
  | { fix: "operator"; tor_socks: string }
  | { fix: "clearnet-on" }
  | { fix: "clearnet-only" };
