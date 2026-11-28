import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import StakeActions from "../StakeActions";
import {
  CONTENT_CHANGED_NOTICE,
  HELD_ADVICE,
  IN_FLIGHT_ADVICE,
  RELEASE_FAILED_ADVICE,
  VERDICT_NOT_SAVED,
} from "../../../types/send";
import type { BuiltPendingTx, SendError } from "../../../types/send";

const BUILT: BuiltPendingTx = { pending_tx_id: "42", fee: "250000000", content_gen: 3 };
const REBUILT: BuiltPendingTx = { pending_tx_id: "43", fee: "350000000", content_gen: 4 };
const MISMATCH: SendError = {
  code: "CONTENT_GEN_MISMATCH",
  message: "content generation mismatch",
  reservation: "held",
};
const AMBIGUOUS: SendError = {
  code: "SUBMIT_AMBIGUOUS",
  message: "daemon submit ambiguous",
  reservation: "in-flight",
};
const DISCARD_FAILED: SendError = {
  code: "INTERNAL_ERROR",
  message: "discard transaction: io",
  reservation: "held",
};
const NOT_SAVED: SendError = {
  code: "SAVE_NOT_WRITABLE",
  message:
    "the send was not saved, so it was not sent — the wallet's folder is not writable; make it writable for this account and submit again, or discard the send",
  reservation: "held",
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

  /**
   * A fund that was not saved sent nothing; its reservation is still the
   * page's (finding on #74). The review stays, Fund stake submits the same
   * reservation again, and Cancel would release it. Red edit: treat `held`
   * as in flight (the review is gone, its inputs locked).
   */
  it("keeps a held reservation under review, to fund again or cancel", async () => {
    let submits = 0;
    route({
      stake_in: () => BUILT,
      submit_pending_tx: () => {
        submits += 1;
        if (submits === 1) throw NOT_SAVED;
        return { tx_hash: "ab".repeat(32), verdict: "ACCEPTED", verdict_saved: true, confirmed_height: null };
      },
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    expect(await screen.findByText(new RegExp(HELD_ADVICE))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    expect(calls("submit_pending_tx")).toHaveLength(2);
    expect(calls("submit_pending_tx")[1]?.[1]).toEqual({ pendingTxId: "42", seenGen: 3 });
    expect(calls("stake_in")).toHaveLength(1);
  });

  /**
   * A fund the node took but whose answer the wallet could not save says
   * the node has it, that it must not be sent again, and that the wallet
   * asks about it at reopen, the same sentence Send shows (finding on #80).
   * Red edit: the plain "Funds sent" line for every accepted fund.
   */
  it("says a fund whose answer was not saved must not be sent again", async () => {
    route({
      stake_in: () => BUILT,
      submit_pending_tx: () => ({
        tx_hash: "ab".repeat(32),
        verdict: "ACCEPTED",
        verdict_saved: false,
        confirmed_height: null,
      }),
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    expect(
      await screen.findByText(
        `Funds sent. They join your stake after the network confirms. ${VERDICT_NOT_SAVED}`,
      ),
    ).toBeInTheDocument();
  });

  it("does not discard a reservation in flight", async () => {
    route({
      stake_in: () => BUILT,
      submit_pending_tx: () => {
        throw AMBIGUOUS;
      },
    });
    const user = await review();
    await user.click(screen.getByRole("button", { name: "Fund stake" }));
    expect(await screen.findByText(new RegExp(IN_FLIGHT_ADVICE))).toBeInTheDocument();
    expect(calls("discard_pending_tx")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: "Fund stake" })).not.toBeInTheDocument();
  });

  it("refuses a fund of zero before it reserves anything", async () => {
    route({ stake_in: () => BUILT });
    const user = userEvent.setup();
    render(<StakeActions />);
    await user.type(fundAmount(), "0");
    await user.click(screen.getByRole("button", { name: "Review" }));
    expect(await screen.findByText("The amount must be greater than zero.")).toBeInTheDocument();
    expect(calls("stake_in")).toHaveLength(0);
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

describe("StakeActions release", () => {
  it("says a stake not yet sent was withdrawn, with no transaction", async () => {
    const user = userEvent.setup();
    route({ unstake: () => ({ verdict: "WITHDRAWN" }) });
    render(<StakeActions />);
    await user.click(screen.getByRole("button", { name: "Release stake" }));
    await user.click(screen.getByRole("button", { name: "Release permanently" }));
    expect(await screen.findByText(/had not been sent yet, so it was withdrawn/)).toBeInTheDocument();
  });

  it("names the posted release by its transaction", async () => {
    const user = userEvent.setup();
    route({ unstake: () => ({ verdict: "BROADCAST", tx_hash: "ab".repeat(32) }) });
    render(<StakeActions />);
    await user.click(screen.getByRole("button", { name: "Release stake" }));
    await user.click(screen.getByRole("button", { name: "Release permanently" }));
    expect(await screen.findByText(/Release posted \(/)).toBeInTheDocument();
  });
});

