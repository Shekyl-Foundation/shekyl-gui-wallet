import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BalanceCard from "../BalanceCard";
import type { Balance } from "../../types/daemon";

const FULL: Balance = {
  balance: "5200000000",
  spendable: "5000000000",
  incoming: "200000000",
  incoming_wait: 4,
  outgoing: "0",
  unspendable: "0",
  staked: "1500000000",
  claimable_rewards: "9007199254740993",
};

const DASHES = 5; // the headline, two legs, two staking figures

/** Core's constants as `get_chain_constants` serves them: a 120-second block. */
const CONSTANTS = { coinbase_lock_window: 60, block_spacing: 120 };

function mockBalance(...reads: Array<Balance | Error | "hang">) {
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === "get_chain_constants") return Promise.resolve(CONSTANTS);
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
    expect(await screen.findByText("5.200000 SKL")).toBeInTheDocument(); // the balance headline
    expect(screen.getByText("5.000000 SKL")).toBeInTheDocument(); // spendable
    expect(screen.getByText("0.200000 SKL")).toBeInTheDocument(); // incoming
    expect(await screen.findByTestId("incoming-wait")).toHaveTextContent(
      "first spendable in about 8 minutes",
    ); // 4 blocks of 120 s, as a time
    expect(screen.queryByText(/parked/i)).not.toBeInTheDocument();
    expect(screen.getByText("1.500000 SKL")).toBeInTheDocument();
    expect(screen.getByText("9007199.254740 SKL")).toBeInTheDocument();
    expect(screen.queryByTestId("unspendable")).not.toBeInTheDocument();
    expect(screen.queryByTestId("outgoing")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows the incoming wait as a time, never as blocks", async () => {
    mockBalance({ ...FULL, incoming_wait: 1 });
    render(<BalanceCard />);
    const wait = await screen.findByTestId("incoming-wait");
    expect(wait).toHaveTextContent("first spendable in about 2 minutes");
    expect(wait).not.toHaveTextContent(/block/i);
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
    mockBalance({ balance: "40", spendable: "30", incoming: "10", outgoing: "0", unspendable: "0" });
    render(<BalanceCard />);
    await waitFor(() => {
      expect(screen.getAllByText("Unavailable")).toHaveLength(2);
    });
    expect(screen.getAllByText("0.000000 SKL")).toHaveLength(3); // balance and the two legs: formatted, not "Unavailable"
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
    expect(await screen.findByText("5.200000 SKL")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "get_balance")).toHaveLength(2);
  });
});
