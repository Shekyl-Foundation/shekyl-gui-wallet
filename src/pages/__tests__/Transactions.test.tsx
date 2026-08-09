import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  statusLabel,
  statusTitle,
  type TxStatus,
} from "../../lib/transactionStatus";
import Transactions from "../Transactions";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

function sampleTx(
  overrides: Partial<{
    id: string;
    hash: string;
    amount: number;
    fee: number;
    height: number | null;
    direction: "in" | "out";
    status: TxStatus;
  }> = {},
) {
  const hash =
    overrides.hash ??
    "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
  return {
    id: overrides.id ?? hash,
    hash,
    amount: 1_000_000_000,
    fee: 10_000,
    height: null as number | null,
    timestamp: 0,
    direction: "out" as const,
    status: "pending" as TxStatus,
    pqc_protected: true,
    ...overrides,
  };
}

describe("status helpers", () => {
  it("labels every lifecycle arm distinctly", () => {
    expect(statusLabel("confirmed")).toBe("Confirmed");
    expect(statusLabel("pending")).toBe("Pending");
    expect(statusLabel("failed")).toBe("Failed");
    expect(statusLabel("dropped")).toBe("Dropped");
    expect(statusLabel("abandoned")).toBe("Abandoned");
    expect(statusLabel("spent")).toBe("Spent");
  });

  it("gives failed, dropped, and abandoned actionable titles (rule 82)", () => {
    expect(statusTitle("failed")).toMatch(/never mined/i);
    expect(statusTitle("dropped")).toMatch(/spendable again/i);
    expect(statusTitle("abandoned")).toMatch(/stop tracking/i);
    expect(statusTitle("pending")).toBeUndefined();
    expect(statusTitle("confirmed")).toBeUndefined();
    expect(statusTitle("spent")).toBeUndefined();
  });
});

describe("Transactions", () => {
  it("renders outgoing pending and failed/dropped without collapsing status", async () => {
    vi.mocked(invoke).mockResolvedValue([
      sampleTx({ status: "pending", height: null }),
      sampleTx({
        hash: "11".repeat(32),
        status: "failed",
        height: null,
      }),
      sampleTx({
        hash: "22".repeat(32),
        status: "dropped",
        height: null,
      }),
      sampleTx({
        id: `${"33".repeat(32)}:0`,
        hash: "33".repeat(32),
        status: "confirmed",
        height: 42,
        direction: "in",
        fee: 0,
      }),
      sampleTx({
        id: `${"44".repeat(32)}:0`,
        hash: "44".repeat(32),
        status: "spent",
        height: 40,
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
    expect(screen.getByText("Spent")).toBeInTheDocument();
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

  it("discards a slower older response so status is not overwritten", async () => {
    let resolveSlow: (value: unknown) => void = () => {};
    const slow = new Promise((resolve) => {
      resolveSlow = resolve;
    });

    vi.mocked(invoke)
      .mockImplementationOnce(() => slow as Promise<unknown>)
      .mockResolvedValueOnce([
        sampleTx({ status: "confirmed", height: 9 }),
      ]);

    render(<Transactions />);

    // Second load (focus or a second schedule) must be able to complete first.
    // Drive it by calling focus after the first invoke is pending.
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(screen.getByText("Confirmed")).toBeInTheDocument();
    });
    expect(screen.getByText("Block 9")).toBeInTheDocument();

    // Stale first response resolves later with pending — must not clobber.
    await act(async () => {
      resolveSlow([sampleTx({ status: "pending", height: null })]);
      await Promise.resolve();
    });
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(screen.queryByText("Pending")).not.toBeInTheDocument();
  });

  it("polls get_transactions so status can advance without remount", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke)
      .mockResolvedValueOnce([sampleTx({ status: "pending" })])
      .mockResolvedValueOnce([sampleTx({ status: "confirmed", height: 9 })]);

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
