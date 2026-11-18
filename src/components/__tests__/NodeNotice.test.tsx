import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import NodeNotice from "../NodeNotice";
import type { NodeStatus } from "../../types/node";

const FOUND =
  "A Shekyl node of yours is already running on testnet. This wallet uses only the node it starts itself.";

function node(over: Partial<NodeStatus>): NodeStatus {
  return {
    state: "running",
    message: null,
    network: "testnet",
    can_force_stop: true,
    can_stop_waiting: false,
    payout_fault: null,
    service_view: null,
    ...over,
  };
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

  /**
   * The start that follows a found node's stop can be ended, so it is
   * said at once with its acts (finding on #71: `starting` hid "Stop
   * waiting" while the backend still took it). Red edit: return nothing
   * for every `starting`.
   */
  it("offers to stop waiting during the start that replaces a found node", async () => {
    mockNode(node({ state: "starting", can_force_stop: true, can_stop_waiting: true }));
    render(<NodeNotice />);
    expect(await screen.findByText("Starting the wallet’s node…")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Stop waiting"));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("stop_waiting_for_node"),
    );
    expect(screen.getByText("Force stop")).toBeInTheDocument();
  });

  /**
   * A plain start is unsaid while it is short, and said with a force stop
   * once it has run long: a start that hangs is never a window with no
   * way out. Red edit: never set `longStart`.
   */
  it("says a start gone long and offers the force stop", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mockNode(node({ state: "starting", can_force_stop: true }));
      const { container } = render(<NodeNotice />);
      await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("daemon_status"));
      expect(container).toBeEmptyDOMElement();
      await vi.advanceTimersByTimeAsync(12000);
      expect(await screen.findByText("Starting the wallet’s node…")).toBeInTheDocument();
      expect(screen.queryByText("Stop waiting")).toBeNull();
      fireEvent.click(screen.getByText("Force stop"));
      await waitFor(() =>
        expect(vi.mocked(invoke)).toHaveBeenCalledWith("force_stop_node"),
      );
    } finally {
      vi.useRealTimers();
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

  it("names the held data directory while waiting and offers to stop waiting", async () => {
    const words =
      "The node you stopped still holds the data directory /home/a/.shekyl/testnet. The wallet's node starts when it lets go.";
    mockNode(
      node({ state: "waiting", message: words, can_force_stop: false, can_stop_waiting: true }),
    );
    render(<NodeNotice />);
    expect(await screen.findByText(words)).toBeInTheDocument();
    expect(screen.queryByText("Force stop")).toBeNull();
    fireEvent.click(screen.getByText("Stop waiting"));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("stop_waiting_for_node"),
    );
  });

  it("offers to stop waiting for a found node that has not gone", async () => {
    mockNode(node({ state: "stopping", can_force_stop: false, can_stop_waiting: true }));
    render(<NodeNotice />);
    expect(await screen.findByText("Stopping the node…")).toBeInTheDocument();
    expect(screen.getByText("Stop waiting")).toBeInTheDocument();
    expect(screen.queryByText("Force stop")).toBeNull();
  });

  it("keeps Stop waiting pressable while the replacement it ends is running", async () => {
    let status = node({ state: "found", message: FOUND, can_force_stop: false });
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "daemon_status") return status;
      // The replacement does not return while it waits.
      if (cmd === "replace_found_node") return new Promise(() => {});
      return null;
    });
    render(<NodeNotice />);
    fireEvent.click(await screen.findByText("Stop it and start the wallet’s own"));
    status = node({
      state: "waiting",
      message: "The node you stopped still holds the data directory /d.",
      can_force_stop: false,
      can_stop_waiting: true,
    });
    const stop = await screen.findByText("Stop waiting", {}, { timeout: 4000 });
    expect(stop).toBeEnabled();
    fireEvent.click(stop);
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("stop_waiting_for_node"),
    );
  });

  it("says why the node is idle after the wait was ended, and offers a retry", async () => {
    const words =
      "The wallet's node is not started: another program still holds the data directory /d.";
    mockNode(node({ state: "idle", message: words, can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByText(words)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Retry"));
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("start_node"));
  });

  it("says a service node in core's words and offers to use it", async () => {
    const words =
      "The node on this computer runs as a service. Until wallets reach a service over the channel, set daemon-address = 127.0.0.1:12030";
    mockNode(node({ state: "service", message: words, can_force_stop: false, service_view: "127.0.0.1:12030" }));
    render(<NodeNotice />);
    expect(await screen.findByText(words)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Use the service’s node"));
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("use_service_node"));
  });

  it("offers nothing to use for a service with no view listener", async () => {
    mockNode(node({ state: "service", message: "no view listener", can_force_stop: false }));
    render(<NodeNotice />);
    expect(await screen.findByText("no view listener")).toBeInTheDocument();
    expect(screen.queryByText("Use the service’s node")).toBeNull();
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
