import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PreWalletNode from "../PreWalletNode";
import type { NodeStatus } from "../../types/node";

const FOUND =
  "A Shekyl node of yours is already running on mainnet. This wallet uses only the node it starts itself.";

function status(over: Partial<NodeStatus>): NodeStatus {
  return {
    state: "found",
    message: FOUND,
    network: "mainnet",
    can_force_stop: false,
    can_stop_waiting: false,
    payout_fault: null,
    service_view: null,
    ...over,
  };
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("PreWalletNode", () => {
  /**
   * Before any wallet is open the node's notice and its acts are there, so
   * a found node, a failed start or a held directory can be dealt with
   * where a wallet is created or opened. Red edit: drop `<NodeNotice />`.
   */
  it("shows the node's notice and its acts before a wallet is open", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) =>
      cmd === "daemon_status" ? status({}) : null,
    );
    render(<PreWalletNode />);
    expect(await screen.findByText(FOUND)).toBeInTheDocument();
    expect(screen.getByText("Stop it and start the wallet’s own")).toBeInTheDocument();
  });

  /** The network a wallet will be made on can be chosen before one is open. */
  it("switches the node's network before a wallet is open", async () => {
    let current = { url: null, warnings: [], network: "mainnet" };
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_status") return status({ state: "running", message: null });
      if (cmd === "daemon_connection_disclosures") return current;
      if (cmd === "set_daemon_connection") {
        current = { url: null, warnings: [], network: (args as { network: string }).network };
        return current;
      }
      return null;
    });
    render(<PreWalletNode />);
    const mainnet = await screen.findByRole("button", { name: "mainnet" });
    await waitFor(() => expect(mainnet).toHaveAttribute("aria-pressed", "true"));
    await userEvent.click(screen.getByRole("button", { name: "testnet" }));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "testnet",
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "testnet" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
  });

  /**
   * A read of the connection asked before a switch and answered after it is
   * stale: the switch's answer stands, and the bar stays on the new network
   * (finding on #80). Red edit: drop the generation check from the read.
   */
  it("keeps a switch over a read answered after it", async () => {
    let answerHeld: (value: unknown) => void = () => {};
    let reads = 0;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_status") return status({ state: "running", message: null });
      if (cmd === "daemon_connection_disclosures") {
        reads += 1;
        if (reads === 1) return { url: null, warnings: [], network: "mainnet" };
        // The second read is held, and answered with the old network after
        // the switch has been applied.
        return new Promise((resolve) => {
          answerHeld = resolve;
        });
      }
      if (cmd === "set_daemon_connection") {
        return { url: null, warnings: [], network: (args as { network: string }).network };
      }
      return null;
    });
    render(<PreWalletNode />);
    const mainnet = await screen.findByRole("button", { name: "mainnet" });
    await waitFor(() => expect(mainnet).toHaveAttribute("aria-pressed", "true"));
    await waitFor(() => expect(reads).toBe(2), { timeout: 4000 });

    await userEvent.click(screen.getByRole("button", { name: "testnet" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "testnet" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    await act(async () => {
      answerHeld({ url: null, warnings: [], network: "mainnet" });
    });
    expect(screen.getByRole("button", { name: "testnet" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "mainnet" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  /**
   * A node address can be named before a wallet is open (finding on #71):
   * a service with no view listener refuses every open, and Settings is
   * reached only with a wallet open, so without this there is no way to a
   * node that works. The address goes with the network in effect, and a
   * refusal is said. Red edit: drop the editor from the bar.
   */
  it("names a node address before a wallet is open", async () => {
    let current: { url: string | null; warnings: string[]; network: string } = {
      url: null,
      warnings: [],
      network: "testnet",
    };
    let refuse = false;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_status") {
        return status({ state: "service", message: "no view listener", network: "testnet" });
      }
      if (cmd === "daemon_connection_disclosures") return current;
      if (cmd === "set_daemon_connection") {
        if (refuse) throw "That address is not a node address.";
        const { url, network } = args as { url?: string; network: string };
        current = { url: url ?? null, warnings: [], network };
        return current;
      }
      return null;
    });
    render(<PreWalletNode />);
    expect(await screen.findByText("Node: this computer’s")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Change node" }));
    const field = screen.getByPlaceholderText("Empty: this computer’s node");
    const save = screen.getByRole("button", { name: /save & reconnect/i });
    await waitFor(() => expect(save).toBeEnabled());
    await userEvent.type(field, "http://127.0.0.1:12030");
    await userEvent.click(save);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_daemon_connection", {
      network: "testnet",
      url: "http://127.0.0.1:12030",
    });
    expect(await screen.findByText("Node: http://127.0.0.1:12030")).toBeInTheDocument();

    refuse = true;
    await userEvent.click(screen.getByRole("button", { name: "Use this computer’s node" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "That address is not a node address.",
    );
  });
});
