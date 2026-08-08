import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Transactions, { statusLabel, statusTitle } from "../Transactions";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

function sampleTx(
  overrides: Partial<{
    hash: string;
    amount: number;
    fee: number;
    height: number;
    direction: string;
    status: string;
    confirmed: boolean;
  }> = {},
) {
  return {
    hash: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
    amount: 1_000_000_000,
    fee: 10_000,
    height: 0,
    timestamp: 0,
    direction: "out",
    status: "pending",
    confirmed: false,
    pqc_protected: true,
    ...overrides,
  };
}

describe("status helpers", () => {
  it("labels every send-journal arm distinctly", () => {
    expect(statusLabel("confirmed")).toBe("Confirmed");
    expect(statusLabel("pending")).toBe("Pending");
    expect(statusLabel("failed")).toBe("Failed");
    expect(statusLabel("dropped")).toBe("Dropped");
  });

  it("gives failed and dropped actionable titles (rule 82)", () => {
    expect(statusTitle("failed")).toMatch(/never mined/i);
    expect(statusTitle("dropped")).toMatch(/spendable again/i);
    expect(statusTitle("pending")).toBeUndefined();
    expect(statusTitle("confirmed")).toBeUndefined();
  });
});

describe("Transactions", () => {
  it("renders outgoing pending and failed/dropped without collapsing status", async () => {
    vi.mocked(invoke).mockResolvedValue([
      sampleTx({ status: "pending", confirmed: false, height: 0 }),
      sampleTx({
        hash: "11".repeat(32),
        status: "failed",
        confirmed: false,
        height: 0,
      }),
      sampleTx({
        hash: "22".repeat(32),
        status: "dropped",
        confirmed: false,
        height: 0,
      }),
      sampleTx({
        hash: "33".repeat(32),
        status: "confirmed",
        confirmed: true,
        height: 42,
        direction: "in",
        fee: 0,
      }),
    ]);

    render(<Transactions />);

    await waitFor(() => {
      expect(screen.getByText("Pending")).toBeInTheDocument();
    });
    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("Dropped")).toBeInTheDocument();
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(screen.getByText("Block 42")).toBeInTheDocument();
    expect(screen.getByTitle(/never mined/i)).toBeInTheDocument();
    expect(screen.getByTitle(/spendable again/i)).toBeInTheDocument();
  });

  it("surfaces a load failure with retry instead of an empty list", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke)
      .mockRejectedValueOnce("wallet not open")
      .mockResolvedValueOnce([]);

    render(<Transactions />);

    await waitFor(() => {
      expect(screen.getByText("wallet not open")).toBeInTheDocument();
    });
    expect(screen.queryByText("No transactions yet")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /try again/i }));

    await waitFor(() => {
      expect(screen.getByText("No transactions yet")).toBeInTheDocument();
    });
    expect(screen.queryByText("wallet not open")).not.toBeInTheDocument();
  });

  it("polls get_transactions so status can advance without remount", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke)
      .mockResolvedValueOnce([sampleTx({ status: "pending" })])
      .mockResolvedValueOnce([
        sampleTx({ status: "confirmed", confirmed: true, height: 9 }),
      ]);

    render(<Transactions />);

    // Flush the initial invoke microtask under fake timers (no waitFor —
    // waitFor advances real time and races the 5s test timeout).
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("Pending")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(screen.getByText("Block 9")).toBeInTheDocument();
  });
});
