import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import NodeNotice from "../NodeNotice";
import type { NodeStatus } from "../../types/node";

const FOUND =
  "A Shekyl node of yours is already running on testnet. This wallet uses only the node it starts itself.";

function node(over: Partial<NodeStatus>): NodeStatus {
  return { state: "running", message: null, network: "testnet", can_force_stop: true, ...over };
}

/** `daemon_status` answers `status`; every other command resolves. */
function mockNode(status: NodeStatus, acts: Record<string, () => unknown> = {}) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "daemon_status") return status;
    if (cmd in acts) return acts[cmd]();
    return null;
  });
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("NodeNotice", () => {
  it("says nothing while the wallet's node is starting or running", async () => {
    for (const state of ["idle", "starting", "running"] as const) {
      mockNode(node({ state }));
      const { container, unmount } = render(<NodeNotice />);
      await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("daemon_status"));
      expect(container).toBeEmptyDOMElement();
      unmount();
    }
  });

  it("says a found node in the backend's words and offers to replace it", async () => {
    mockNode(node({ state: "found", message: FOUND, can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByText(FOUND)).toBeInTheDocument();
    // Not the wallet's node: nothing here kills it.
    expect(screen.queryByText("Force stop")).toBeNull();
    fireEvent.click(screen.getByText("Stop it and start the wallet’s own"));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("replace_found_node"),
    );
  });

  it("shows why the replacement did not happen", async () => {
    mockNode(node({ state: "found", message: FOUND, can_force_stop: false }), {
      replace_found_node: () => {
        throw "Your node on testnet did not take the stop: it did not answer";
      },
    });
    render(<NodeNotice />);
    fireEvent.click(await screen.findByText("Stop it and start the wallet’s own"));
    expect(
      await screen.findByText("Your node on testnet did not take the stop: it did not answer"),
    ).toBeInTheDocument();
  });

  it("says a stopping node and offers the force stop throughout", async () => {
    mockNode(node({ state: "stopping", can_force_stop: true }));
    render(<NodeNotice />);
    expect(await screen.findByText("Stopping the node…")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Force stop"));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("force_stop_node"),
    );
  });

  it("offers no force stop for a stopping node the wallet did not start", async () => {
    mockNode(node({ state: "stopping", can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByText("Stopping the node…")).toBeInTheDocument();
    expect(screen.queryByText("Force stop")).toBeNull();
  });

  it("says a failed start in the node's own words and offers a start", async () => {
    const words = "shekyld: the data directory is in use by another process";
    mockNode(node({ state: "failed", message: words, can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByText(words)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Start the node"));
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("start_node"));
  });

  it("says a failed peer check as an alert, with no act to take", async () => {
    const words =
      "Something other than your Shekyl node is answering at its local address. Not connecting. (owner)";
    mockNode(node({ state: "refused", message: words, can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByRole("alert")).toHaveTextContent(words);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
