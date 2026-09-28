import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { statusLabel, statusTitle } from "../../lib/transactionStatus";
import type { Transfer, TransferState, Transfers, UnspendableReason } from "../../types/transfers";
import Transactions from "../Transactions";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

/** One projected `Transfer` row; amounts are decimal strings on the wire. */
function sampleTx(
  overrides: {
    id?: string;
    tx_hash?: string;
    amount?: string;
    fee?: string;
    block_height?: number;
    direction?: Transfer["direction"];
    state?: TransferState;
    unspendable_reason?: UnspendableReason;
  } = {},
): Transfer {
  const tx_hash =
    overrides.tx_hash ?? "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
  const common = {
    id: overrides.id ?? tx_hash,
    tx_hash,
    amount: overrides.amount ?? "1000000000",
    fee: overrides.fee ?? "10000",
    block_height: overrides.block_height,
    direction: overrides.direction ?? "OUTGOING",
  };
  if (overrides.state === "UNSPENDABLE") {
    return {
      ...common,
      state: "UNSPENDABLE",
      unspendable_reason: overrides.unspendable_reason ?? "PQC_LEAF_MISMATCH",
    };
  }
  return { ...common, state: overrides.state ?? "PENDING" };
}

const transfers = (rows: Transfer[]): Transfers => ({ transfers: rows });

describe("state helpers", () => {
  it("labels every contract state distinctly", () => {
    expect(statusLabel("CONFIRMED")).toBe("Confirmed");
    expect(statusLabel("PENDING")).toBe("Pending");
    expect(statusLabel("FAILED")).toBe("Failed");
    expect(statusLabel("DROPPED")).toBe("Dropped");
    expect(statusLabel("ABANDONED")).toBe("Abandoned");
    expect(statusLabel("SPENT")).toBe("Spent");
    expect(statusLabel("UNSPENDABLE")).toBe("Unspendable");
  });

  it("gives failed, dropped, abandoned and unspendable actionable titles (rule 82)", () => {
    expect(statusTitle("FAILED")).toMatch(/never mined/i);
    expect(statusTitle("DROPPED")).toMatch(/spendable again/i);
    expect(statusTitle("ABANDONED")).toMatch(/stop tracking/i);
    expect(statusTitle("UNSPENDABLE", "PQC_LEAF_MISMATCH")).toMatch(/not created for this wallet/i);
    expect(statusTitle("UNSPENDABLE", "PQC_LEAF_ENTRY_ABSENT")).toMatch(/missing what a spend needs/i);
    expect(statusTitle("UNSPENDABLE")).toMatch(/never spend/i);
    expect(statusTitle("PENDING")).toBeUndefined();
    expect(statusTitle("CONFIRMED")).toBeUndefined();
    expect(statusTitle("SPENT")).toBeUndefined();
  });
});

describe("Transactions", () => {
  it("renders outgoing pending and failed/dropped without collapsing state", async () => {
    vi.mocked(invoke).mockResolvedValue(
      transfers([
        sampleTx({ state: "PENDING" }),
        sampleTx({ tx_hash: "11".repeat(32), state: "FAILED" }),
        sampleTx({ tx_hash: "22".repeat(32), state: "DROPPED" }),
        sampleTx({
          id: `${"33".repeat(32)}:0`,
          tx_hash: "33".repeat(32),
          state: "CONFIRMED",
          block_height: 42,
          direction: "INCOMING",
          fee: "0",
        }),
        sampleTx({
          id: `${"44".repeat(32)}:0`,
          tx_hash: "44".repeat(32),
          state: "SPENT",
          block_height: 40,
          direction: "INCOMING",
          fee: "0",
        }),
      ]),
    );

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
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_transfers");
  });

  it("renders an unspendable receive with the reason the projection named", async () => {
    vi.mocked(invoke).mockResolvedValue(
      transfers([
        sampleTx({
          direction: "INCOMING",
          state: "UNSPENDABLE",
          unspendable_reason: "PQC_LEAF_ENTRY_ABSENT",
          block_height: 7,
          fee: "0",
        }),
      ]),
    );
    render(<Transactions />);
    expect(await screen.findByText("Unspendable")).toBeInTheDocument();
    expect(screen.getByText("Block 7")).toBeInTheDocument();
    expect(screen.getByTitle(/missing what a spend needs/i)).toBeInTheDocument();
  });

  it("renders amounts above 2^53 atomic units exactly", async () => {
    vi.mocked(invoke).mockResolvedValue(
      transfers([sampleTx({ amount: "9007199254740993", fee: "1", state: "CONFIRMED", block_height: 1 })]),
    );
    render(<Transactions />);
    expect(await screen.findByText("-9007199.254740 SKL")).toBeInTheDocument();
  });

  it("surfaces a load failure with retry instead of an empty list", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValueOnce("wallet not open").mockResolvedValueOnce(transfers([]));

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

  it("discards a slower older response so state is not overwritten", async () => {
    let resolveSlow: (value: unknown) => void = () => {};
    const slow = new Promise((resolve) => {
      resolveSlow = resolve;
    });

    vi.mocked(invoke)
      .mockImplementationOnce(() => slow as Promise<unknown>)
      .mockResolvedValueOnce(transfers([sampleTx({ state: "CONFIRMED", block_height: 9 })]));

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
      resolveSlow(transfers([sampleTx({ state: "PENDING" })]));
      await Promise.resolve();
    });
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(screen.queryByText("Pending")).not.toBeInTheDocument();
  });

  it("polls get_transfers so state can advance without remount", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke)
      .mockResolvedValueOnce(transfers([sampleTx({ state: "PENDING" })]))
      .mockResolvedValueOnce(transfers([sampleTx({ state: "CONFIRMED", block_height: 9 })]));

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
