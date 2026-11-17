import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { ReactNode } from "react";
import { TorContext } from "../../context/torState";
import PostureLine from "../PostureLine";
import TorSettings from "../TorSettings";
import ReviewCard from "../send/ReviewCard";
import TransactionHistory from "../transactions/TransactionHistory";
import {
  HELD_LABEL,
  acceptedStatement,
  maySubmit,
  pendingIsHeld,
  reviewStatement,
  sendsNow,
  type SendsNow,
} from "../../lib/sendWords";
import type { OfferedFix, Sends, TorRead } from "../../types/tor";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

/** Core's rendering of a held node, as the backend passes it through. */
const HELD_TEXT =
  "This node is waiting for Tor and holding its own transactions. It is syncing over clearnet.";

function read(sends: Sends, over: Partial<TorRead> = {}): TorRead {
  return { shown: sends !== "withheld", text: HELD_TEXT, sends, fixes: [], ...over };
}

function withTor(tor: TorRead | null, children: ReactNode, refresh = () => {}) {
  return render(
    <TorContext.Provider value={{ read: tor, error: null, refresh }}>{children}</TorContext.Provider>,
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(openDialog).mockReset();
});

describe("the header's posture line", () => {
  it("says nothing while a send leaves hidden", () => {
    const { container } = withTor(read("hidden"), <PostureLine />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows core's words, verbatim, whenever a send would not leave hidden", () => {
    for (const sends of ["held", "open-network", "offline", "unknown", "withheld"] as const) {
      const { unmount } = withTor(read(sends), <PostureLine />);
      expect(screen.getByTestId("posture-line")).toHaveTextContent(HELD_TEXT);
      unmount();
    }
  });

  it("claims nothing about a node it could not read", () => {
    const { container } = withTor(null, <PostureLine />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("what the wallet says about a send", () => {
  const EVERY: SendsNow[] = [
    "hidden",
    "held",
    "open-network",
    "offline",
    "unknown",
    "withheld",
    "unread",
  ];

  it("adds nothing to a hidden send, and something to every other", () => {
    for (const now of EVERY) {
      expect(reviewStatement(now) === null).toBe(now === "hidden");
    }
  });

  /** Held promises no clearnet; nothing else may borrow that promise. */
  it("promises that a send stays off the open network only when it is held", () => {
    for (const now of EVERY) {
      const promised = (reviewStatement(now) ?? "").includes("will not go out over the open network");
      expect(promised).toBe(now === "held");
      expect(pendingIsHeld(now)).toBe(now === "held");
    }
    expect(acceptedStatement("held")).toContain("waiting on your node");
    expect(acceptedStatement("open-network")).toBe(
      "It is going out over the open network, not through Tor.",
    );
    // Core's ClearnetOnly sentence says the same of that network.
    expect(reviewStatement("open-network")).toContain(
      "Until the encrypted link is in place, that network is unencrypted.",
    );
    expect(acceptedStatement("hidden")).toBeNull();
  });

  it("does not offer an offline node's send for confirming", () => {
    for (const now of EVERY) {
      expect(maySubmit(now)).toBe(now !== "offline");
    }
    expect(sendsNow(null)).toBe("unread");
    expect(sendsNow(read("held"))).toBe("held");
  });

  it("puts the statement on the review card", () => {
    render(<ReviewCard address="shekyl1abc" amount={1n} fee={1n} statement={reviewStatement("held")} />);
    expect(screen.getByTestId("send-statement")).toHaveTextContent(
      "This one will wait on your node until Tor carries it.",
    );
  });
});

describe("pending sends while the node holds", () => {
  const rows = {
    transfers: [
      {
        id: "aa".repeat(32),
        tx_hash: "aa".repeat(32),
        amount: "1000000000",
        fee: "10000",
        direction: "OUTGOING",
        state: "PENDING",
      },
      {
        id: `${"bb".repeat(32)}:0`,
        tx_hash: "bb".repeat(32),
        amount: "1000000000",
        fee: "0",
        direction: "INCOMING",
        state: "PENDING",
        attribution: { kind: "UNATTRIBUTED" },
      },
    ],
  };
  const query = {
    direction: undefined,
    state: undefined,
    sinceHeight: undefined,
    attribution: undefined,
  };

  it("shows this wallet's pending sends as held, and leaves receives alone", async () => {
    vi.mocked(invoke).mockResolvedValue(rows);
    withTor(read("held"), <TransactionHistory query={query} onLookup={() => {}} />);
    expect(await screen.findByText(HELD_LABEL)).toBeInTheDocument();
    expect(screen.getAllByText("Pending")).toHaveLength(1);
  });

  it("shows them as pending when the node is not holding, or cannot be read", async () => {
    for (const tor of [read("hidden"), read("withheld"), null]) {
      vi.mocked(invoke).mockResolvedValue(rows);
      const { unmount } = withTor(tor, <TransactionHistory query={query} onLookup={() => {}} />);
      await waitFor(() => expect(screen.getAllByText("Pending")).toHaveLength(2));
      expect(screen.queryByText(HELD_LABEL)).toBeNull();
      unmount();
    }
  });
});

describe("Settings → Network", () => {
  const FIXES: OfferedFix[] = [
    {
      keyword: "restore-managed",
      sentence: "Restore the bundled Tor, or name one that passes its check.",
    },
    { keyword: "operator", sentence: "Or use a Tor you run yourself." },
    { keyword: "clearnet-on", sentence: "Or turn clearnet on and keep Tor managed." },
    {
      keyword: "clearnet-only",
      sentence:
        "Or turn Tor off and send over the open network. Until the encrypted link is in place, that network is unencrypted.",
    },
  ];

  it("offers exactly the read's fixes, in its order, each in core's words", () => {
    withTor(read("held", { fixes: FIXES }), <TorSettings inForce={null} onChanged={() => {}} />);
    const items = screen.getAllByRole("listitem");
    expect(items.map((li) => li.querySelector("p")?.textContent)).toEqual(
      FIXES.map((f) => f.sentence),
    );
    // Nothing is pre-selected, and nothing is taken without a press.
    expect(vi.mocked(invoke)).not.toHaveBeenCalled();
  });

  it("offers no fix the read does not", () => {
    withTor(read("held"), <TorSettings inForce={null} onChanged={() => {}} />);
    expect(screen.queryByTestId("tor-fixes")).toBeNull();
    expect(screen.getByTestId("tor-read")).toHaveTextContent(HELD_TEXT);
  });

  it("takes a fix as it stands: the operator's SOCKS address is passed unjudged", async () => {
    const changed = vi.fn();
    const refresh = vi.fn();
    vi.mocked(invoke).mockResolvedValue(null);
    withTor(
      read("held", { fixes: FIXES }),
      <TorSettings inForce={null} onChanged={changed} />,
      refresh,
    );
    const u = userEvent.setup();
    await u.type(screen.getByPlaceholderText("127.0.0.1:9050"), "not an address");
    await u.click(screen.getByRole("button", { name: "Use my own Tor" }));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_tor_fix", {
      fix: { fix: "operator", tor_socks: "not an address" },
    });
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(refresh).toHaveBeenCalled();
  });

  it("names a Tor program with the system file chooser", async () => {
    vi.mocked(invoke).mockResolvedValue(null);
    vi.mocked(openDialog).mockResolvedValue("/opt/tor/tor");
    withTor(read("held", { fixes: FIXES }), <TorSettings inForce={null} onChanged={() => {}} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Choose a Tor program…" }));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_tor_fix", {
        fix: { fix: "restore-managed", tor_file: "/opt/tor/tor" },
      }),
    );
  });

  it("takes nothing when the chooser is dismissed", async () => {
    vi.mocked(openDialog).mockResolvedValue(null);
    withTor(read("held", { fixes: FIXES }), <TorSettings inForce={null} onChanged={() => {}} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Choose a Tor program…" }));
    await waitFor(() => expect(vi.mocked(openDialog)).toHaveBeenCalled());
    expect(vi.mocked(invoke)).not.toHaveBeenCalled();
  });

  it("shows the node's refusal in its own words, and keeps the way back", async () => {
    const refusal =
      "shekyld: `tor-socks` is not an endpoint: not an address [config.value.endpoint from command-line]";
    vi.mocked(invoke).mockRejectedValue(refusal);
    // The node did not start: no read at all. The fix in force is the
    // wallet's own setting, so the way back is still there.
    withTor(
      null,
      <TorSettings inForce={{ fix: "operator", tor_socks: "not an address" }} onChanged={() => {}} />,
    );
    const back = screen.getByRole("button", { name: "Use the bundled Tor again" });
    await userEvent.setup().click(back);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_tor_fix", { fix: null });
    expect(await screen.findByRole("alert")).toHaveTextContent(refusal);
    expect(screen.getByRole("button", { name: "Use the bundled Tor again" })).toBeInTheDocument();
  });

  it("offers the way back from Tor off, where the read offers no fix", () => {
    withTor(
      read("open-network"),
      <TorSettings inForce={{ fix: "clearnet-only" }} onChanged={() => {}} />,
    );
    expect(screen.getByTestId("tor-fix-in-force")).toHaveTextContent("You turned Tor off");
    expect(screen.getByRole("button", { name: "Go back to the bundled Tor" })).toBeInTheDocument();
    expect(screen.queryByTestId("tor-fixes")).toBeNull();
  });
});
