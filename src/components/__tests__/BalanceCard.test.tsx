import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BalanceCard from "../BalanceCard";
import type { Balance } from "../../types/daemon";

const FULL: Balance = {
  liquid: "5000000000",
  unlocked: "5000000000",
  pending: "250000000",
  unspendable: "0",
  staked: "1500000000",
  claimable_rewards: "9007199254740993",
};

function mockBalance(balance: Balance | (() => Promise<never>)) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_balance") return typeof balance === "function" ? balance() : balance;
    throw new Error(`unexpected: ${cmd}`);
  });
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("BalanceCard", () => {
  it("renders the contract's figures as SKL, exactly", async () => {
    mockBalance(FULL);
    render(<BalanceCard />);
    await waitFor(() => {
      expect(screen.getByText("5.000000 SKL")).toBeInTheDocument();
    });
    expect(screen.getByText("0.250000 SKL")).toBeInTheDocument();
    expect(screen.getByText("1.500000 SKL")).toBeInTheDocument();
    expect(screen.getByText("9007199.254740 SKL")).toBeInTheDocument();
    expect(screen.queryByTestId("unspendable")).not.toBeInTheDocument();
  });

  it("shows an unspendable total only when there is one", async () => {
    mockBalance({ ...FULL, unspendable: "7" });
    render(<BalanceCard />);
    expect(await screen.findByTestId("unspendable")).toHaveTextContent("Unspendable: 0.000000 SKL");
  });

  it("renders absent staking figures as unavailable, never as zero", async () => {
    mockBalance({ liquid: "40", unlocked: "40", pending: "0", unspendable: "0" });
    render(<BalanceCard />);
    await waitFor(() => {
      expect(screen.getAllByText("Unavailable")).toHaveLength(2);
    });
    expect(screen.getAllByText("0.000000 SKL")).toHaveLength(2); // liquid and pending, formatted, not "Unavailable"
    expect(screen.getAllByTitle(/could not be read/i)).toHaveLength(2);
  });

  it("shows placeholder dashes before data loads and when the read fails", async () => {
    mockBalance(() => new Promise(() => {}));
    const view = render(<BalanceCard />);
    expect(screen.getAllByText("— SKL")).toHaveLength(4);
    view.unmount();

    mockBalance(() => Promise.reject(new Error("No wallet is open")));
    render(<BalanceCard />);
    await new Promise((r) => setTimeout(r, 10));
    expect(screen.getAllByText("— SKL")).toHaveLength(4);
  });
});
