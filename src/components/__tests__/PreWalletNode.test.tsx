import { render, screen, waitFor } from "@testing-library/react";
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
    let current = status({ state: "running", message: null });
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "daemon_status") return current;
      if (cmd === "set_daemon_connection") {
        current = status({
          state: "running",
          message: null,
          network: (args as { network: string }).network,
        });
        return { url: null, warnings: [] };
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
});
