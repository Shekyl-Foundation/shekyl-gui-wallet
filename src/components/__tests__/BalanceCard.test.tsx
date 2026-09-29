import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BalanceCard from "../BalanceCard";
import type { Balance } from "../../types/daemon";

const FULL: Balance = {
  liquid: "5250000000",
  unlocked: "5000000000",
  pending: "250000000",
  unspendable: "0",
  staked: "1500000000",
  claimable_rewards: "9007199254740993",
};

const DASHES = 5;

function mockBalance(...reads: Array<Balance | Error | "hang">) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd !== "get_balance") return Promise.reject(new Error(`unexpected: ${cmd}`));
    const read = reads.length > 1 ? reads.shift()! : reads[0];
    if (read === "hang") return new Promise(() => {});
    return read instanceof Error ? Promise.reject(read) : Promise.resolve(read);
  });
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("BalanceCard", () => {
  it("renders the contract's figures as SKL, exactly, each under its own label", async () => {
    mockBalance(FULL);
    render(<BalanceCard />);
    expect(await screen.findByText("5.250000 SKL")).toBeInTheDocument(); // liquid headline
    expect(screen.getByText("5.000000 SKL")).toBeInTheDocument(); // unlocked = Available
    expect(screen.getByText("0.250000 SKL")).toBeInTheDocument();
    expect(screen.getByText("1.500000 SKL")).toBeInTheDocument();
    expect(screen.getByText("9007199.254740 SKL")).toBeInTheDocument();
    expect(screen.queryByTestId("unspendable")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows an unspendable total only when there is one, at dust precision", async () => {
    mockBalance({ ...FULL, unspendable: "7" });
    render(<BalanceCard />);
    expect(await screen.findByTestId("unspendable")).toHaveTextContent("Unspendable: 0.000000007 SKL");
  });

  it("renders absent staking figures as unavailable, never as zero", async () => {
    mockBalance({ liquid: "40", unlocked: "30", pending: "0", unspendable: "0" });
    render(<BalanceCard />);
    await waitFor(() => {
      expect(screen.getAllByText("Unavailable")).toHaveLength(2);
    });
    expect(screen.getAllByText("0.000000 SKL")).toHaveLength(3); // liquid, unlocked, pending: formatted, not "Unavailable"
    expect(screen.getAllByTitle(/could not be read/i)).toHaveLength(2);
  });

  it("shows placeholder dashes while the read is in flight", () => {
    mockBalance("hang");
    render(<BalanceCard />);
    expect(screen.getAllByText("— SKL")).toHaveLength(DASHES);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a failed read with the engine's message and recovers on retry", async () => {
    mockBalance(new Error("No wallet is open"), FULL);
    render(<BalanceCard />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Balance could not be read: No wallet is open");
    expect(screen.getAllByText("— SKL")).toHaveLength(DASHES);

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("5.250000 SKL")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledTimes(2);
  });
});
