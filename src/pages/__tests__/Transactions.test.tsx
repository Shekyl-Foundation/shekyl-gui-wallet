import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { statusLabel, statusTitle } from "../../lib/transactionStatus";
import type {
  ReceiveAttribution,
  Transfer,
  TransferState,
  Transfers,
  UnspendableReason,
} from "../../types/transfers";
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
    attribution?: ReceiveAttribution;
  } = {},
): Transfer {
  const tx_hash =
    overrides.tx_hash ?? "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
  const shared = {
    id: overrides.id ?? tx_hash,
    tx_hash,
    amount: overrides.amount ?? "1000000000",
    fee: overrides.fee ?? "10000",
    block_height: overrides.block_height,
  };
  const direction = overrides.direction ?? "OUTGOING";
  if (direction === "INCOMING") {
    const attribution = overrides.attribution ?? { kind: "UNATTRIBUTED" as const };
    if (overrides.state === "UNSPENDABLE") {
      return {
        ...shared,
        direction,
        attribution,
        state: "UNSPENDABLE",
        unspendable_reason: overrides.unspendable_reason ?? "PQC_LEAF_MISMATCH",
      };
    }
    return {
      ...shared,
      direction,
      attribution,
      state: overrides.state ?? "CONFIRMED",
    };
  }
  return {
    ...shared,
    direction: "OUTGOING",
    state: overrides.state && overrides.state !== "UNSPENDABLE" ? overrides.state : "PENDING",
  };
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

    const rows = () => within(screen.getByTestId("transfers"));
    await waitFor(() => {
      expect(rows().getByText("Pending")).toBeInTheDocument();
    });
    expect(rows().getByText("Failed")).toBeInTheDocument();
    expect(rows().getByText("Dropped")).toBeInTheDocument();
    expect(rows().getByText("Confirmed")).toBeInTheDocument();
    expect(rows().getByText("Spent")).toBeInTheDocument();
    expect(rows().getByText("Block 42")).toBeInTheDocument();
    expect(screen.getByTitle(/never mined/i)).toBeInTheDocument();
    expect(screen.getByTitle(/spendable again/i)).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_transfers", {
      direction: undefined,
      state: undefined,
    });
  });

  it("sends the contract's direction and state filters to Rust, never filtering a shown list itself", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockResolvedValue(transfers([sampleTx({ state: "PENDING" })]));
    render(<Transactions />);
    await waitFor(() => expect(within(screen.getByTestId("transfers")).getByText("Pending")).toBeInTheDocument());
    await user.click(screen.getByRole("tab", { name: "Received" }));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenLastCalledWith("get_transfers", {
        direction: "INCOMING",
        state: undefined,
      }),
    );
    await user.selectOptions(screen.getByLabelText("State"), "CONFIRMED");
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenLastCalledWith("get_transfers", {
        direction: "INCOMING",
        state: "CONFIRMED",
      }),
    );
  });

  it("hides the previous rows while a new filter is in flight, then says when nothing matches", async () => {
    const user = userEvent.setup();
    let resolveFiltered: (value: unknown) => void = () => {};
    const filtered = new Promise((resolve) => {
      resolveFiltered = resolve;
    });
    vi.mocked(invoke)
      .mockResolvedValueOnce(transfers([sampleTx({ state: "PENDING" })]))
      .mockImplementationOnce(() => filtered);

    render(<Transactions />);
    expect(await screen.findByText("Pending")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Received" }));
    expect(screen.queryByTestId("transfers")).not.toBeInTheDocument();
    expect(screen.getByText("Loading transactions…")).toBeInTheDocument();

    await act(async () => {
      resolveFiltered(transfers([]));
      await Promise.resolve();
    });
    expect(await screen.findByText("No matching transactions")).toBeInTheDocument();
    expect(screen.queryByText("No transactions yet")).not.toBeInTheDocument();
  });

  it("shows which payment request a receive arrived against", async () => {
    vi.mocked(invoke).mockResolvedValue(
      transfers([
        sampleTx({
          id: `${"55".repeat(32)}:0`,
          tx_hash: "55".repeat(32),
          direction: "INCOMING",
          state: "CONFIRMED",
          block_height: 7,
          fee: "0",
          attribution: { kind: "MATCHED", request_id: "42" },
        }),
        sampleTx({
          id: `${"66".repeat(32)}:0`,
          tx_hash: "66".repeat(32),
          direction: "INCOMING",
          state: "CONFIRMED",
          block_height: 8,
          fee: "0",
          attribution: { kind: "UNATTRIBUTED" },
        }),
      ]),
    );
    render(<Transactions />);
    expect(await screen.findByText("Request 42")).toBeInTheDocument();
    expect(screen.getAllByTestId("attribution")).toHaveLength(1);
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

    const rows = () => within(screen.getByTestId("transfers"));
    await waitFor(() => {
      expect(rows().getByText("Confirmed")).toBeInTheDocument();
    });
    expect(rows().getByText("Block 9")).toBeInTheDocument();

    // Stale first response resolves later with pending — must not clobber.
    await act(async () => {
      resolveSlow(transfers([sampleTx({ state: "PENDING" })]));
      await Promise.resolve();
    });
    expect(rows().getByText("Confirmed")).toBeInTheDocument();
    expect(rows().queryByText("Pending")).not.toBeInTheDocument();
  });

  it("polls get_transfers so state can advance without remount", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke)
      .mockResolvedValueOnce(transfers([sampleTx({ state: "PENDING" })]))
      .mockResolvedValueOnce(transfers([sampleTx({ state: "CONFIRMED", block_height: 9 })]));

    render(<Transactions />);

    // Flush the initial invoke microtask under fake timers (no waitFor —
    // waitFor advances real time and races the 5s test timeout).
    const rows = () => within(screen.getByTestId("transfers"));
    await act(async () => {
      await Promise.resolve();
    });
    expect(rows().getByText("Pending")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(rows().getByText("Confirmed")).toBeInTheDocument();
    expect(rows().getByText("Block 9")).toBeInTheDocument();
  });
});
