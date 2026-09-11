import { render, screen, waitFor } from "@testing-library/react";
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
