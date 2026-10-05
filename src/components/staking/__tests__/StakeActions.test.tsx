import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import StakeActions from "../StakeActions";
import { CONTENT_CHANGED_NOTICE, RELEASE_FAILED_ADVICE, RETAINED_ADVICE } from "../../../types/send";
import type { BuiltPendingTx, SendError } from "../../../types/send";

const BUILT: BuiltPendingTx = { pending_tx_id: "42", fee: "250000000", content_gen: 3 };
const REBUILT: BuiltPendingTx = { pending_tx_id: "43", fee: "350000000", content_gen: 4 };
const MISMATCH: SendError = {
  code: "CONTENT_GEN_MISMATCH",
  message: "content generation mismatch",
  reservation_retained: true,
};
const AMBIGUOUS: SendError = {
  code: "SUBMIT_AMBIGUOUS",
  message: "daemon submit ambiguous",
  reservation_retained: true,
};
const DISCARD_FAILED: SendError = {
  code: "INTERNAL_ERROR",
  message: "discard transaction: io",
  reservation_retained: true,
};

type Handler = (args?: Record<string, unknown>) => unknown;

function route(handlers: Record<string, Handler>) {
  vi.mocked(invoke).mockImplementation((cmd, args) => {
    const handler = handlers[cmd as string];
    if (!handler) return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    try {
      return Promise.resolve(handler(args as Record<string, unknown>));
    } catch (err) {
      return Promise.reject(err);
    }
  });
}

const calls = (cmd: string) => vi.mocked(invoke).mock.calls.filter(([name]) => name === cmd);

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function fundAmount() {
  const fields = screen.getAllByPlaceholderText("Amount (SKL)");
  return fields[0];
}

async function review() {
  const user = userEvent.setup();
  render(<StakeActions />);
  await user.type(fundAmount(), "1");
  await user.click(screen.getByRole("button", { name: "Review" }));
  expect(await screen.findByText(/Fee:/)).toBeInTheDocument();
  return user;
}

describe("StakeActions fund reservation", () => {
  it("discards the reservation when the panel is left", async () => {
    route({ stake_in: () => BUILT });
    const { unmount } = render(<StakeActions />);
    const user = userEvent.setup();
    await user.type(fundAmount(), "1");
    await user.click(screen.getByRole("button", { name: "Review" }));
    await screen.findByText(/Fee:/);
    unmount();
    expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]);
  });

  it("stays on review when cancel cannot release the reservation", async () => {
    route({
      stake_in: () => BUILT,
      discard_pending_tx: () => {
        throw DISCARD_FAILED;
      },
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText(new RegExp(RELEASE_FAILED_ADVICE))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fund stake" })).toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(1);
  });

  it("does not discard a reservation the engine kept", async () => {
    route({
      stake_in: () => BUILT,
      submit_pending_tx: () => {
        throw AMBIGUOUS;
      },
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    expect(await screen.findByText(new RegExp(RETAINED_ADVICE))).toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: "Fund stake" })).not.toBeInTheDocument();
  });

  it("discards and rebuilds when the chain moves during review", async () => {
    let builds = 0;
    route({
      stake_in: () => (builds++ === 0 ? BUILT : REBUILT),
      submit_pending_tx: () => {
        throw MISMATCH;
      },
      discard_pending_tx: () => ({}),
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    const notice = await screen.findByText(CONTENT_CHANGED_NOTICE);
    expect(notice).toHaveClass("text-amber-100");
    expect(notice).not.toHaveClass("text-emerald-200");
    expect(await screen.findByText(/0\.350000 SKL/)).toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(1);
    expect(calls("stake_in")).toHaveLength(2);
    expect(calls("stake_in")[1]?.[1]).toEqual({ amount: "1000000000" });
  });

  it("does not claim the review was rebuilt when the new one fails", async () => {
    let builds = 0;
    route({
      stake_in: () => {
        builds += 1;
        if (builds === 1) return BUILT;
        throw new Error("stake build failed");
      },
      submit_pending_tx: () => {
        throw MISMATCH;
      },
      discard_pending_tx: () => ({}),
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    const failure = await screen.findByText(/The earlier review was released/);
    expect(failure).toHaveClass("text-red-300");
    expect(failure).toHaveTextContent("stake build failed");
    expect(screen.queryByText(CONTENT_CHANGED_NOTICE)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fund stake" })).not.toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(1);
  });
});

describe("StakeActions collect", () => {
  it("says nothing remains only when the pass finished the lane", async () => {
    const user = userEvent.setup();
    const cases: Array<{ result: unknown; text: string }> = [
      { result: { status: "NOTHING_LEFT" }, text: "Nothing left to collect." },
      {
        result: {
          status: "SWEPT",
          tx_hash: "ab",
          swept: "1",
          remainder: "0",
          another_pool_remains: true,
        },
        text: "Released funds from another stake still remain.",
      },
      {
        result: {
          status: "SWEPT",
          tx_hash: "ab",
          swept: "1",
          remainder: "5",
          another_pool_remains: false,
        },
        text: "Some funds are not spendable yet.",
      },
      {
        result: {
          status: "SWEPT",
          tx_hash: "ab",
          swept: "1",
          remainder: "0",
          another_pool_remains: false,
        },
        text: "nothing further remains.",
      },
    ];
    for (const item of cases) {
      route({ collect_unstaked: () => item.result });
      const view = render(<StakeActions />);
      await user.click(screen.getByRole("button", { name: "Collect" }));
      expect(await screen.findByText(new RegExp(item.text))).toBeInTheDocument();
      view.unmount();
    }
  });
});
