import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Settings from "../Settings";

/** The §1 operator statement, as the backend returns it. */
const OPERATOR_WARNING =
  "Warning: daemon URL 'node.example.com' is not a loopback address. " +
  "Whoever operates that daemon sees which blocks this wallet requests…";

/**
 * `warnings` is what the backend establishes about the configured daemon
 * URL: empty for a daemon on this machine, the operator statement for one
 * that is not loopback.
 */
function mockDaemon(warnings: string[], applied: string[] = warnings) {
  // The connection in effect, as the backend keeps it: a later read of it
  // gives what the last save set.
  let current: { url: string | null; warnings: string[]; network: string } = {
    url: null,
    warnings,
    network: "mainnet",
  };
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === "daemon_connection_disclosures") return current;
    if (cmd === "set_daemon_connection") {
      const { url, network } = args as { url?: string; network: string };
      current = url
        ? { url, warnings: applied, network }
        : { url: null, warnings, network };
      return current;
    }
    return null;
  });
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function renderSettings() {
  return render(
    <DaemonProvider>
      <Settings />
    </DaemonProvider>,
  );
}

describe("Settings — daemon disclosure", () => {
  it("says nothing about a daemon on this machine", async () => {
    mockDaemon([]);
    renderSettings();
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "daemon_connection_disclosures",
      ),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  /**
   * A wallet that starts pointed at a daemon it does not own says so on
   * sight — not only in the session where the URL was typed.
   */
  it("shows the operator statement for a daemon already configured", async () => {
    mockDaemon([OPERATOR_WARNING]);
    renderSettings();
    expect(
      await screen.findByText(/is not a loopback address/),
    ).toBeInTheDocument();
  });

  /**
   * The field is what "Save & Reconnect" writes back, so it has to show the
   * URL actually in effect. Showing the default beside a warning about the
   * configured daemon would revert the operator's choice the next time they
   * pressed the button. The edit that turns this red is dropping
   * `setDaemonUrl(current.url)` from the load path.
   */
  it("loads the daemon URL in effect, not the default", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_connection_disclosures") {
        return {
          url: "http://node.example.com:11029/json_rpc",
          warnings: [OPERATOR_WARNING],
          network: "mainnet",
        };
      }
      return null;
    });
    renderSettings();
    const field = await screen.findByDisplayValue(
      "http://node.example.com:11029/json_rpc",
    );
    expect(field).toBeInTheDocument();
  });

  /** The point of configuration: it is said as the URL is saved. */
  it("shows the operator statement when a non-loopback URL is saved", async () => {
    mockDaemon([], [OPERATOR_WARNING]);
    renderSettings();
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith(
        "daemon_connection_disclosures",
      ),
    );
    expect(screen.queryByRole("status")).toBeNull();

    const field = await screen.findByPlaceholderText("Empty: this computer’s node");
    await userEvent.type(field, "http://node.example.com:11029");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /save & reconnect/i })).toBeEnabled(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: /save & reconnect/i }),
    );
    expect(
      await screen.findByText(/is not a loopback address/),
    ).toBeInTheDocument();
  });
});

describe("Settings — which node", () => {
  /**
   * This computer's node shows no address (it is read over its owner's
   * leg): saving an empty field keeps it, and no URL is sent. A typed
   * address is sent, and the way back is offered. Red edit: send the
   * field on every save, empty included.
   */
  it("sends a URL only when one was typed", async () => {
    mockDaemon([]);
    renderSettings();
    const save = await screen.findByRole("button", { name: /save & reconnect/i });
    const field = await screen.findByPlaceholderText("Empty: this computer’s node");
    expect(field).toHaveValue("");
    await waitFor(() => expect(save).toBeEnabled());
    await userEvent.click(save);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "mainnet",
    });
    expect(screen.getByText("This computer’s node, the one the wallet starts.")).toBeInTheDocument();

    await userEvent.type(field, "http://node.example.com:11029");
    await userEvent.click(save);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "mainnet",
      url: "http://node.example.com:11029",
    });
    expect(
      await screen.findByText("A node address you typed: the wallet asks it over the network."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use this computer’s node" })).toBeInTheDocument();
  });
});

describe("Settings — network", () => {
  it("switches by network alone, keeping this computer's node", async () => {
    mockDaemon([]);
    renderSettings();
    const testnet = await screen.findByRole("button", { name: "testnet" });
    await waitFor(() => expect(testnet).toBeEnabled());
    fireEvent.click(testnet);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "testnet" })).toHaveClass("border-gold-500"),
    );
    expect(screen.getByPlaceholderText("Empty: this computer’s node")).toHaveValue("");
    // No URL rides the switch: the wallet's node follows the network.
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "testnet",
    });
  });

  it("shows why a switch waits while a wallet is open", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: null, warnings: [], network: "testnet" };
      }
      if (cmd === "set_daemon_connection") {
        throw "The open wallet is on testnet. Close it before switching to mainnet: a wallet belongs to one network.";
      }
      return null;
    });
    renderSettings();
    const mainnet = await screen.findByRole("button", { name: "mainnet" });
    await waitFor(() => expect(mainnet).toBeEnabled());
    fireEvent.click(mainnet);
    expect(
      await screen.findByText(/Close it before switching to mainnet/),
    ).toBeInTheDocument();
  });

  /**
   * The network is the backend's, read with the connection, and never the
   * node's health (finding on #71): with the node away there is no health,
   * and a testnet wallet's page must still say testnet and save a node
   * address on testnet. Red edit: take the network from `health`, falling
   * back to mainnet (the save then names mainnet, which the backend
   * refuses for a testnet wallet).
   */
  it("takes the network from the backend, with no node answering", async () => {
    let asked: unknown = null;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: null, warnings: [], network: "testnet" };
      }
      if (cmd === "get_chain_health") throw "The wallet's node is not running.";
      if (cmd === "set_daemon_connection") {
        asked = args;
        return { url: (args as { url: string }).url, warnings: [], network: "testnet" };
      }
      return null;
    });
    renderSettings();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "testnet" })).toHaveClass("border-gold-500"),
    );
    expect(screen.getByRole("button", { name: "mainnet" })).not.toHaveClass("border-gold-500");
    await userEvent.type(
      screen.getByPlaceholderText("Empty: this computer’s node"),
      "http://127.0.0.1:12030",
    );
    await userEvent.click(screen.getByRole("button", { name: /save & reconnect/i }));
    await waitFor(() =>
      expect(asked).toEqual({ network: "testnet", url: "http://127.0.0.1:12030" }),
    );
  });

  /**
   * A node address the backend refuses is said in its words, and the field
   * keeps what was typed (finding on #71: the refusal was dropped). With a
   * wallet open the node it uses does not change until it is closed. Red
   * edit: an empty `catch` in the editor's save.
   */
  it("shows why a node address was not saved", async () => {
    const refusal =
      "Close the wallet before changing the node it uses: an open wallet keeps the node it was opened on.";
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: null, warnings: [], network: "testnet" };
      }
      if (cmd === "set_daemon_connection") throw refusal;
      return null;
    });
    renderSettings();
    const field = await screen.findByPlaceholderText("Empty: this computer’s node");
    await userEvent.type(field, "http://127.0.0.1:12030");
    const save = screen.getByRole("button", { name: /save & reconnect/i });
    await waitFor(() => expect(save).toBeEnabled());
    await userEvent.click(save);
    expect(await screen.findByRole("alert")).toHaveTextContent(refusal);
    expect(field).toHaveValue("http://127.0.0.1:12030");
  });
});

describe("Settings — the wallet's node", () => {
  const NOT_YET =
    "Keeping the node running after the wallet closes is not yet implemented. " +
    "The node this wallet starts stops when the wallet closes.";

  /** Core's rendering, as the backend passes it through. */
  const TOR_TEXT =
    "Your transactions wait on this node until Tor can carry them. " +
    "Peers are known; none has answered through Tor for 2 minutes.";

  let inbound = false;

  function mockNode(state: string) {
    inbound = false;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: null, warnings: [], network: "testnet" };
      }
      if (cmd === "get_daemon_settings") return { data_dir: null, clearnet_inbound: inbound };
      if (cmd === "daemon_status") {
        return { state, message: null, network: "testnet", can_force_stop: state === "running", can_stop_waiting: false, payout_fault: null, service_view: null };
      }
      if (cmd === "set_daemon_settings") {
        const asked = args as Record<string, unknown>;
        if ("clearnetInbound" in asked) {
          inbound = asked.clearnetInbound as boolean;
          return { data_dir: null, clearnet_inbound: inbound };
        }
        expect(asked).toEqual({ keepRunningOnExit: true });
        throw NOT_YET;
      }
      if (cmd === "get_tor_status") return { shown: true, text: TOR_TEXT };
      return null;
    });
  }

  it("says where the wallet's node stands, and on which network", async () => {
    mockNode("running");
    renderSettings();
    expect(
      await screen.findByText("The wallet’s node is running (testnet)"),
    ).toBeInTheDocument();
  });

  it("says a found node is not the wallet's own", async () => {
    mockNode("found");
    renderSettings();
    expect(
      await screen.findByText(
        "Another node of yours is running; the wallet’s own is not (testnet)",
      ),
    ).toBeInTheDocument();
  });

  /**
   * The setting stays where people look for it and answers in the backend's
   * words; it never shows as on.
   */
  it("answers the keep-running switch with not yet implemented", async () => {
    mockNode("running");
    renderSettings();
    const toggle = await screen.findByRole("switch", {
      name: "Keep daemon running after wallet closes",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(await screen.findByText(NOT_YET)).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  /**
   * The inbound clearnet listener is off by default and can be turned on:
   * the switch asks the backend, which keeps it and starts the node again
   * (Rick, 2026-11-17).
   */
  it("starts with incoming clearnet connections off and turns them on", async () => {
    mockNode("running");
    renderSettings();
    const toggle = await screen.findByRole("switch", {
      name: "Accept incoming connections over clearnet",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_settings", {
      clearnetInbound: true,
    });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
  });

  /** The Tor read is the backend's text, shown as it came. */
  it("shows the node's Tor state in the backend's words", async () => {
    mockNode("running");
    renderSettings();
    expect(await screen.findByTestId("tor-read")).toHaveTextContent(TOR_TEXT);
  });

  /**
   * A read slower than the page's beat is still shown (finding on #71).
   * The Tor read takes longer than the two seconds between reads: the next
   * read is asked for only once this one has finished, so it is never
   * numbered out by a read begun behind it. Red edit: read on a fixed
   * `setInterval` (each slow answer is dropped for the next, for ever).
   */
  it("shows a Tor read slower than its beat", async () => {
    mockNode("running");
    const answer = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "get_tor_status") {
        await new Promise((done) => setTimeout(done, 2600));
      }
      return answer(cmd, args as never);
    });
    renderSettings();
    expect(
      await screen.findByTestId("tor-read", {}, { timeout: 6000 }),
    ).toHaveTextContent(TOR_TEXT);
  }, 10000);
});
