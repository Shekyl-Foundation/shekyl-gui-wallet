import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BalanceCard from "../BalanceCard";
import type { Balance } from "../../types/daemon";

const FULL: Balance = {
  balance: "5250000000",
  spendable: "5000000000",
  incoming: "200000000",
  incoming_wait: 4,
  parked: "50000000",
  outgoing: "0",
  unspendable: "0",
  staked: "1500000000",
  claimable_rewards: "9007199254740993",
};

const DASHES = 6; // the headline, three legs, two staking figures

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
    expect(await screen.findByText("5.250000 SKL")).toBeInTheDocument(); // the balance headline
    expect(screen.getByText("5.000000 SKL")).toBeInTheDocument(); // spendable
    expect(screen.getByText("0.200000 SKL")).toBeInTheDocument(); // incoming
    expect(screen.getByTestId("incoming-wait")).toHaveTextContent("first spendable in 4 blocks");
    expect(screen.getByText("0.050000 SKL")).toBeInTheDocument(); // parked
    expect(screen.getByText("1.500000 SKL")).toBeInTheDocument();
    expect(screen.getByText("9007199.254740 SKL")).toBeInTheDocument();
    expect(screen.queryByTestId("unspendable")).not.toBeInTheDocument();
    expect(screen.queryByTestId("outgoing")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows an unspendable total only when there is one, at dust precision", async () => {
    mockBalance({ ...FULL, unspendable: "7" });
    render(<BalanceCard />);
    expect(await screen.findByTestId("unspendable")).toHaveTextContent("Unspendable: 0.000000007 SKL");
  });

  it("shows sends in flight beside the balance, only when there are some", async () => {
    mockBalance({ ...FULL, outgoing: "300000000" });
    render(<BalanceCard />);
    expect(await screen.findByTestId("outgoing")).toHaveTextContent("Outgoing: 0.300000 SKL");
  });

  it("renders absent staking figures as unavailable, never as zero", async () => {
    mockBalance({ balance: "40", spendable: "30", incoming: "10", parked: "0", outgoing: "0", unspendable: "0" });
    render(<BalanceCard />);
    await waitFor(() => {
      expect(screen.getAllByText("Unavailable")).toHaveLength(2);
    });
    expect(screen.getAllByText("0.000000 SKL")).toHaveLength(4); // balance and the three legs: formatted, not "Unavailable"
    expect(screen.queryByTestId("incoming-wait")).not.toBeInTheDocument(); // absent, not 0
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
