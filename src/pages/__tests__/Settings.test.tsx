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
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "daemon_connection_disclosures") {
      return { url: "http://127.0.0.1:11029/json_rpc", warnings };
    }
    if (cmd === "set_daemon_connection") {
      return { url: "http://node.example.com:11029", warnings: applied };
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

    await userEvent.click(
      screen.getByRole("button", { name: /save & reconnect/i }),
    );
    expect(
      await screen.findByText(/is not a loopback address/),
    ).toBeInTheDocument();
  });
});

describe("Settings — network", () => {
  it("switches by network alone and shows the URL the backend chose", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: "http://127.0.0.1:11029/json_rpc", warnings: [] };
      }
      if (cmd === "set_daemon_connection") {
        return { url: "http://127.0.0.1:12029/json_rpc", warnings: [] };
      }
      return null;
    });
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "testnet" }));
    await waitFor(() => {
      expect(screen.getByDisplayValue("http://127.0.0.1:12029/json_rpc")).toBeInTheDocument();
    });
    // No URL rides the switch: the port is core's, chosen by the backend.
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "testnet",
    });
  });

  it("shows why a switch waits while a wallet is open", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: "http://127.0.0.1:12029/json_rpc", warnings: [] };
      }
      if (cmd === "set_daemon_connection") {
        throw "The open wallet is on testnet. Close it before switching to mainnet: a wallet belongs to one network.";
      }
      return null;
    });
    renderSettings();
    fireEvent.click(await screen.findByRole("button", { name: "mainnet" }));
    expect(
      await screen.findByText(/Close it before switching to mainnet/),
    ).toBeInTheDocument();
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

  function mockNode(state: string) {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_connection_disclosures") {
        return { url: "http://127.0.0.1:12029/json_rpc", warnings: [] };
      }
      if (cmd === "get_daemon_settings") return { data_dir: null };
      if (cmd === "daemon_status") {
        return { state, message: null, network: "testnet", can_force_stop: state === "running", can_stop_waiting: false };
      }
      if (cmd === "set_daemon_settings") {
        expect(args).toEqual({ keepRunningOnExit: true });
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
    const toggle = await screen.findByRole("switch");
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await userEvent.click(toggle);
    expect(await screen.findByText(NOT_YET)).toBeInTheDocument();
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  /** The Tor read is the backend's text, shown as it came. */
  it("shows the node's Tor state in the backend's words", async () => {
    mockNode("running");
    renderSettings();
    expect(await screen.findByTestId("tor-read")).toHaveTextContent(TOR_TEXT);
  });
});
