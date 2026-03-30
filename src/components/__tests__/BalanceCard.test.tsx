import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import BalanceCard from "../BalanceCard";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("BalanceCard", () => {
  it("renders balances formatted with 6-decimal SKL (1e9 atomic)", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_balance") {
        return {
          total: 5_000_000_000,
          unlocked: 3_500_000_000,
          staked: 1_500_000_000,
        };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    render(<BalanceCard />);

    await waitFor(() => {
      expect(screen.getByText("5.000000 SKL")).toBeInTheDocument();
    });
    expect(screen.getByText("3.500000 SKL")).toBeInTheDocument();
    expect(screen.getByText("1.500000 SKL")).toBeInTheDocument();
  });

  it("shows placeholder dashes before data loads", () => {
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));

    render(<BalanceCard />);

    const dashes = screen.getAllByText("— SKL");
    expect(dashes.length).toBe(3);
  });
});
