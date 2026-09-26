import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Send from "../Send";
import type { BuiltPendingTx, FeeTierQuote, SendError, SubmitResult } from "../../types/send";

const QUOTE: FeeTierQuote = {
  default_priority: "STANDARD",
  economy_fee: 100_000_000,
  standard_fee: 200_000_000,
  priority_fee: 400_000_000,
  tree_depth: 6,
};
const BUILT: BuiltPendingTx = { pending_tx_id: "42", fee: 250_000_000, content_gen: 0 };
const REBUILT: BuiltPendingTx = { pending_tx_id: "43", fee: 275_000_000, content_gen: 0 };
const ACCEPTED: SubmitResult = { tx_hash: "ab".repeat(32), verdict: "ACCEPTED", confirmed_height: null };
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

type Handler = (args?: Record<string, unknown>) => unknown;

/** Route `invoke` by command name; unrouted commands reject loudly. */
function route(handlers: Record<string, Handler>) {
  vi.mocked(invoke).mockImplementation((cmd, args) => {
    const h = handlers[cmd as string];
    if (!h) return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    try {
      return Promise.resolve(h(args as Record<string, unknown>));
    } catch (e) {
      return Promise.reject(e);
    }
  });
}

const calls = (cmd: string) => vi.mocked(invoke).mock.calls.filter(([c]) => c === cmd);

async function fillForm(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByPlaceholderText("shekyl1..."), "shekyl1abc123");
  await user.type(screen.getByPlaceholderText("0.0000"), "1.5");
}

async function reachReview(user: ReturnType<typeof userEvent.setup>) {
  await fillForm(user);
  await user.click(screen.getByRole("button", { name: /review/i }));
  await screen.findByTestId("review");
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("Send page", () => {
  it("shows the three tier quotes from one fetch, and the form", async () => {
    route({ get_default_fee_priority: () => QUOTE });
    render(<Send />);
    expect(screen.getByText("Send SKL")).toBeInTheDocument();
    expect(await screen.findByText("≈ 0.2000 SKL")).toBeInTheDocument();
    expect(screen.getByText("≈ 0.1000 SKL")).toBeInTheDocument();
    expect(screen.getByText("≈ 0.4000 SKL")).toBeInTheDocument();
    expect(calls("get_default_fee_priority")).toHaveLength(1);
  });

  it("never builds a transaction while the user is typing", async () => {
    route({ get_default_fee_priority: () => QUOTE });
    const user = userEvent.setup();
    render(<Send />);
    await fillForm(user);
    // Longer than any debounce the old page had; nothing may be built.
    await new Promise((r) => setTimeout(r, 800));
    expect(calls("build_pending_tx")).toHaveLength(0);
    expect(calls("estimate_fee")).toHaveLength(0);
  });

  it("Review builds exactly once with the chosen tier and shows the exact fee", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT });
    const user = userEvent.setup();
    render(<Send />);
    await user.click(screen.getByLabelText(/priority/i));
    await reachReview(user);
    expect(calls("build_pending_tx")).toHaveLength(1);
    expect(calls("build_pending_tx")[0][1]).toEqual({
      address: "shekyl1abc123",
      amount: 1_500_000_000,
      priority: "PRIORITY",
    });
    expect(screen.getByTestId("exact-fee")).toHaveTextContent("0.2500 SKL");
    expect(screen.getByText("1.7500 SKL")).toBeInTheDocument(); // total
  });

  it("Confirm submits that reservation with its content_gen and shows the verdict", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      submit_pending_tx: () => ACCEPTED,
    });
    const user = userEvent.setup();
    render(<Send />);
    await reachReview(user);
    await user.click(screen.getByRole("button", { name: /confirm and send/i }));
    expect(await screen.findByText("Transaction submitted.")).toBeInTheDocument();
    expect(calls("submit_pending_tx")).toEqual([
      ["submit_pending_tx", { pendingTxId: "42", seenGen: 0 }],
    ]);
    expect(calls("discard_pending_tx")).toHaveLength(0);
  });

  it("Cancel discards the reservation and returns to the form", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      discard_pending_tx: () => undefined,
    });
    const user = userEvent.setup();
    render(<Send />);
    await reachReview(user);
    await user.click(screen.getByRole("button", { name: /cancel/i }));
    await waitFor(() => expect(screen.queryByTestId("review")).not.toBeInTheDocument());
    expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]);
    expect(screen.getByRole("button", { name: /review/i })).toBeInTheDocument();
  });

  it("a content change discards, rebuilds once, and never resubmits the stale generation", async () => {
    let submits = 0;
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => (calls("build_pending_tx").length === 1 ? BUILT : REBUILT),
      discard_pending_tx: () => undefined,
      submit_pending_tx: () => {
        submits += 1;
        if (submits === 1) throw MISMATCH;
        return ACCEPTED;
      },
    });
    const user = userEvent.setup();
    render(<Send />);
    await reachReview(user);
    await user.click(screen.getByRole("button", { name: /confirm and send/i }));

    // Back in review with the REBUILT figures, having asked for nothing twice.
    await screen.findByRole("status");
    await waitFor(() => expect(screen.getByTestId("exact-fee")).toHaveTextContent("0.2750 SKL"));
    expect(calls("submit_pending_tx")).toHaveLength(1);
    expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]);
    expect(calls("build_pending_tx")).toHaveLength(2);

    // Only an explicit second confirm submits, and with the new reservation.
    await user.click(screen.getByRole("button", { name: /confirm and send/i }));
    expect(await screen.findByText("Transaction submitted.")).toBeInTheDocument();
    expect(calls("submit_pending_tx")[1][1]).toEqual({ pendingTxId: "43", seenGen: 0 });
  });

  it("leaving the page while reviewing discards the reservation", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      discard_pending_tx: () => undefined,
    });
    const user = userEvent.setup();
    const view = render(<Send />);
    await reachReview(user);
    view.unmount();
    await waitFor(() =>
      expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]),
    );
  });

  it("a build failure is shown and nothing is discarded", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => {
        throw { code: "BUILD_FAILED", message: "insufficient funds", reservation_retained: false };
      },
    });
    const user = userEvent.setup();
    render(<Send />);
    await fillForm(user);
    await user.click(screen.getByRole("button", { name: /review/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("insufficient funds");
    expect(calls("discard_pending_tx")).toHaveLength(0);
    expect(screen.getByRole("button", { name: /review/i })).toBeInTheDocument();
  });

  it("a retained reservation is never discarded by the page", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      submit_pending_tx: () => {
        throw AMBIGUOUS;
      },
    });
    const user = userEvent.setup();
    const view = render(<Send />);
    await reachReview(user);
    await user.click(screen.getByRole("button", { name: /confirm and send/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/check Transactions/);
    view.unmount();
    await new Promise((r) => setTimeout(r, 50));
    expect(calls("discard_pending_tx")).toHaveLength(0);
  });
});
