import type { Sends, TorRead } from "../types/tor";

/**
 * What the wallet says about a send, from the node's one Tor reading
 * (`docs/design/GUI_SYNC_LANE.md` GS-3, GS-F2: held is a state, not a
 * moment). `unread`: the wallet could not read the node at all.
 */
export type SendsNow = Sends | "unread";

export function sendsNow(read: TorRead | null): SendsNow {
  return read ? read.sends : "unread";
}

/** Said on the review card, before the person confirms. `null`: nothing to add. */
const BEFORE: Record<SendsNow, string | null> = {
  hidden: null,
  held: "Your node is holding its own transactions. This one will wait on your node until Tor carries it. It will not go out over the open network.",
  "open-network":
    "Tor is off on your node. This transaction will go out over the open network, not through Tor. Until the encrypted link is in place, that network is unencrypted.",
  offline: "Your node is offline. This transaction cannot be sent now.",
  unknown:
    "Your node reports a Tor condition this version does not know, so the wallet cannot say how this transaction will leave.",
  withheld:
    "This node does not let this wallet see its Tor state, so the wallet cannot say how this transaction will leave.",
  unread:
    "The wallet could not read your node's Tor state, so it cannot say how this transaction will leave.",
};

/** Said after the node accepted the send. `null`: nothing to add. */
const AFTER: Record<SendsNow, string | null> = {
  hidden: null,
  held: "It is waiting on your node until Tor carries it. It will not go out over the open network.",
  "open-network": "It is going out over the open network, not through Tor.",
  offline: "Your node is offline: it has not gone out.",
  unknown: null,
  withheld: null,
  unread: null,
};

export function reviewStatement(now: SendsNow): string | null {
  return BEFORE[now];
}

export function acceptedStatement(now: SendsNow): string | null {
  return AFTER[now];
}

/** A send the node would not send at all is not offered for confirming. */
export function maySubmit(now: SendsNow): boolean {
  return now !== "offline";
}

/** A pending send of this wallet's, while the node holds its own transactions. */
export const HELD_LABEL = "Held on your node";
export const HELD_TITLE =
  "Your node is holding this send until Tor carries it. It will not go out over the open network.";

/** Whether a pending send of this wallet's is shown as held. */
export function pendingIsHeld(now: SendsNow): boolean {
  return now === "held";
}
