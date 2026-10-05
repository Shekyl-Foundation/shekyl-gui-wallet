import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Send from "../Send";
import type { BuiltPendingTx, FeeTierQuote, SendError, SubmitResult } from "../../types/send";

const QUOTE: FeeTierQuote = {
  default_priority: "STANDARD",
  economy_fee: "100000000",
  standard_fee: "200000000",
  priority_fee: "400000000",
  tree_depth: 6,
};
const BUILT: BuiltPendingTx = { pending_tx_id: "42", fee: "250000000", content_gen: 0 };
const REBUILT: BuiltPendingTx = { pending_tx_id: "43", fee: "250000001", content_gen: 0 };
const ACCEPTED: SubmitResult = { tx_hash: "ab".repeat(32), verdict: "ACCEPTED", confirmed_height: null };
const MISMATCH: SendError = { code: "CONTENT_GEN_MISMATCH", message: "content generation mismatch", reservation_retained: true };
const AMBIGUOUS: SendError = { code: "SUBMIT_AMBIGUOUS", message: "daemon submit ambiguous", reservation_retained: true };
const DISCARD_FAILED: SendError = { code: "INTERNAL_ERROR", message: "discard transaction: io", reservation_retained: true };

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
const user = () => userEvent.setup();

async function fillForm(u: ReturnType<typeof userEvent.setup>, amount = "1.5") {
  await u.type(screen.getByPlaceholderText("shekyl1..."), "shekyl1abc123");
  await u.type(screen.getByPlaceholderText("0.0000"), amount);
}

async function reachReview(u: ReturnType<typeof userEvent.setup>, amount?: string) {
  await fillForm(u, amount);
  await u.click(screen.getByRole("button", { name: /review/i }));
  await screen.findByTestId("review");
}

const confirm = (u: ReturnType<typeof userEvent.setup>) =>
  u.click(screen.getByRole("button", { name: /confirm and send/i }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("Send page", () => {
  it("shows the three tier quotes from one fetch, and the form", async () => {
    route({ get_default_fee_priority: () => QUOTE });
    render(<Send />);
    expect(screen.getByText("Send SKL")).toBeInTheDocument();
    expect(await screen.findByText("≈ 0.200000 SKL")).toBeInTheDocument();
    expect(screen.getByText("≈ 0.100000 SKL")).toBeInTheDocument();
    expect(screen.getByText("≈ 0.400000 SKL")).toBeInTheDocument();
    expect(calls("get_default_fee_priority")).toHaveLength(1);
  });

  it("never builds a transaction while the user is typing", async () => {
    route({ get_default_fee_priority: () => QUOTE });
    render(<Send />);
    await fillForm(user());
    await new Promise((r) => setTimeout(r, 800)); // longer than any debounce the old page had
    expect(calls("build_pending_tx")).toHaveLength(0);
  });

  it("Review builds exactly once with the chosen tier and shows the exact fee", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT });
    const u = user();
    render(<Send />);
    await u.click(screen.getByRole("radio", { name: /^priority/i }));
    await reachReview(u);
    expect(calls("build_pending_tx")).toEqual([
      ["build_pending_tx", { address: "shekyl1abc123", amount: "1500000000", priority: "PRIORITY" }],
    ]);
    expect(screen.getByTestId("exact-fee")).toHaveTextContent("0.250000000 SKL");
    expect(screen.getByTestId("total")).toHaveTextContent("1.750000000 SKL");
  });

  it("carries amounts above 2^53 atomic units to Rust losslessly", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT });
    const u = user();
    render(<Send />);
    await reachReview(u, "9007199.254740993");
    expect(calls("build_pending_tx")[0][1]).toMatchObject({ amount: "9007199254740993" });
  });

  it("renders fees that differ by one atomic unit differently", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => REBUILT });
    const u = user();
    render(<Send />);
    await reachReview(u);
    expect(screen.getByTestId("exact-fee")).toHaveTextContent("0.250000001 SKL");
  });

  it("Confirm submits that reservation with its content_gen and shows the verdict", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT, submit_pending_tx: () => ACCEPTED });
    const u = user();
    render(<Send />);
    await reachReview(u);
    await confirm(u);
    expect(await screen.findByText("Transaction submitted.")).toBeInTheDocument();
    expect(calls("submit_pending_tx")).toEqual([["submit_pending_tx", { pendingTxId: "42", seenGen: 0 }]]);
    expect(calls("discard_pending_tx")).toHaveLength(0);
  });

  it("Cancel discards the reservation and returns to the form", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT, discard_pending_tx: () => undefined });
    const u = user();
    render(<Send />);
    await reachReview(u);
    await u.click(screen.getByRole("button", { name: /cancel/i }));
    await waitFor(() => expect(screen.queryByTestId("review")).not.toBeInTheDocument());
    expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]);
    expect(screen.getByRole("button", { name: /review/i })).toBeInTheDocument();
  });

  it("a failed discard keeps the reservation owned: review stays, the failure shows, Cancel retries", async () => {
    let discards = 0;
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      discard_pending_tx: () => {
        discards += 1;
        if (discards === 1) throw DISCARD_FAILED;
        return undefined;
      },
    });
    const u = user();
    render(<Send />);
    await reachReview(u);
    await u.click(screen.getByRole("button", { name: /cancel/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be released/);
    expect(screen.getByTestId("review")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: /cancel/i }));
    await waitFor(() => expect(screen.queryByTestId("review")).not.toBeInTheDocument());
    expect(calls("discard_pending_tx")).toHaveLength(2);
  });

  it("nothing can act on a reservation while its discard is in flight", async () => {
    let releaseDiscard: () => void = () => {};
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      discard_pending_tx: () => new Promise<void>((resolve) => (releaseDiscard = resolve)),
    });
    const u = user();
    render(<Send />);
    await reachReview(u);
    await u.click(screen.getByRole("button", { name: /cancel/i }));
    await waitFor(() => expect(screen.getByRole("button", { name: /confirm and send/i })).toBeDisabled());
    await u.click(screen.getByRole("button", { name: /confirm and send/i }));
    expect(calls("submit_pending_tx")).toHaveLength(0);
    releaseDiscard();
    await waitFor(() => expect(screen.queryByTestId("review")).not.toBeInTheDocument());
    expect(calls("submit_pending_tx")).toHaveLength(0);
    expect(calls("discard_pending_tx")).toHaveLength(1);
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
    const u = user();
    render(<Send />);
    await reachReview(u);
    await confirm(u);
    await screen.findByRole("status");
    await waitFor(() => expect(screen.getByTestId("exact-fee")).toHaveTextContent("0.250000001 SKL"));
    expect(calls("submit_pending_tx")).toHaveLength(1);
    expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]);
    expect(calls("build_pending_tx")).toHaveLength(2);
    await confirm(u);
    expect(await screen.findByText("Transaction submitted.")).toBeInTheDocument();
    expect(calls("submit_pending_tx")[1][1]).toEqual({ pendingTxId: "43", seenGen: 0 });
  });

  it("a content change whose discard fails never stacks a second reservation", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      discard_pending_tx: () => {
        throw DISCARD_FAILED;
      },
      submit_pending_tx: () => {
        throw MISMATCH;
      },
    });
    const u = user();
    render(<Send />);
    await reachReview(u);
    await confirm(u);
    expect(await screen.findByRole("alert")).toHaveTextContent(/could not be released/);
    expect(screen.getByTestId("review")).toBeInTheDocument();
    expect(calls("build_pending_tx")).toHaveLength(1);
    expect(calls("submit_pending_tx")).toHaveLength(1);
  });

  it("leaving the page while reviewing discards the reservation", async () => {
    route({ get_default_fee_priority: () => QUOTE, build_pending_tx: () => BUILT, discard_pending_tx: () => undefined });
    const u = user();
    const view = render(<Send />);
    await reachReview(u);
    view.unmount();
    await waitFor(() => expect(calls("discard_pending_tx")).toEqual([["discard_pending_tx", { pendingTxId: "42" }]]));
  });

  it("a build failure is shown and nothing is discarded", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => {
        throw { code: "INSUFFICIENT_FUNDS", message: "insufficient funds", reservation_retained: false };
      },
    });
    const u = user();
    render(<Send />);
    await fillForm(u);
    await u.click(screen.getByRole("button", { name: /review/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("insufficient funds");
    expect(calls("discard_pending_tx")).toHaveLength(0);
    expect(screen.getByRole("button", { name: /review/i })).toBeInTheDocument();
  });

  it("an amount with more precision than an atomic unit is refused by the form before any build", async () => {
    route({ get_default_fee_priority: () => QUOTE });
    const u = user();
    render(<Send />);
    await fillForm(u, "0.0000000001");
    expect(screen.getByPlaceholderText("0.0000")).toBeInvalid();
    await u.click(screen.getByRole("button", { name: /review/i }));
    await new Promise((r) => setTimeout(r, 50));
    expect(calls("build_pending_tx")).toHaveLength(0);
    expect(screen.queryByTestId("review")).not.toBeInTheDocument();
  });

  it("a pasted payment link fills the address and amount through parse_uri, and says so", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      parse_uri: (args) => {
        expect(args).toEqual({ uri: "shekyl:shekyl1abc123?amount=1500000000&label=Rent&rid=42" });
        return { address: "shekyl1abc123", amount: "1500000000", label: "Rent", rid: "42" };
      },
      build_pending_tx: () => BUILT,
    });
    const u = user();
    render(<Send />);
    await u.click(screen.getByPlaceholderText("shekyl1..."));
    await u.paste("shekyl:shekyl1abc123?amount=1500000000&label=Rent&rid=42");
    await waitFor(() => expect(screen.getByPlaceholderText("shekyl1...")).toHaveValue("shekyl1abc123"));
    expect(screen.getByPlaceholderText("0.0000")).toHaveValue("1.500000000");
    expect(screen.getByTestId("link-notice")).toHaveTextContent(/"Rent".*request 42/);
    await u.click(screen.getByRole("button", { name: /review/i }));
    await screen.findByTestId("review");
    expect(calls("build_pending_tx")[0][1]).toMatchObject({ address: "shekyl1abc123", amount: "1500000000", rid: "42" });
  });

  it("an address edited by hand after a link answers no request", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      parse_uri: () => ({ address: "shekyl1abc123", amount: "1500000000", rid: "42" }),
      build_pending_tx: () => BUILT,
    });
    const u = user();
    render(<Send />);
    const field = screen.getByPlaceholderText("shekyl1...");
    await u.click(field);
    await u.paste("shekyl:shekyl1abc123?amount=1500000000&rid=42");
    await waitFor(() => expect(field).toHaveValue("shekyl1abc123"));
    await u.type(field, "4");
    await u.click(screen.getByRole("button", { name: /review/i }));
    await screen.findByTestId("review");
    expect(calls("build_pending_tx")[0][1]).toMatchObject({ address: "shekyl1abc1234", rid: undefined });
  });

  it("review waits until the payment link has been read, then builds that address and amount", async () => {
    let resolveParse: (value: unknown) => void = () => {};
    route({
      get_default_fee_priority: () => QUOTE,
      parse_uri: () => new Promise((resolve) => (resolveParse = resolve)),
      build_pending_tx: () => BUILT,
    });
    const u = user();
    render(<Send />);
    const field = screen.getByPlaceholderText("shekyl1...");
    await u.click(field);
    await u.type(field, "shekyl1old");
    await u.type(screen.getByPlaceholderText("0.0000"), "1");
    await u.clear(field);
    await u.paste("shekyl:shekyl1new?amount=2000000000");
    expect(field).toHaveValue("shekyl:shekyl1new?amount=2000000000");
    expect(screen.getByRole("button", { name: /reading link/i })).toBeDisabled();
    fireEvent.submit(field.closest("form")!);
    expect(calls("build_pending_tx")).toHaveLength(0);
    resolveParse({ address: "shekyl1new", amount: "2000000000", label: "Rent" });
    await waitFor(() => expect(field).toHaveValue("shekyl1new"));
    expect(screen.getByPlaceholderText("0.0000")).toHaveValue("2.000000000");
    await u.click(screen.getByRole("button", { name: /^review$/i }));
    await screen.findByTestId("review");
    expect(calls("build_pending_tx")[0][1]).toMatchObject({ address: "shekyl1new", amount: "2000000000" });
  });

  it("a parse that resolves for an older paste never overwrites a newer one", async () => {
    let resolveFirst: (v: unknown) => void = () => {};
    let parses = 0;
    route({
      get_default_fee_priority: () => QUOTE,
      parse_uri: () => {
        parses += 1;
        if (parses === 1) return new Promise((resolve) => (resolveFirst = resolve));
        return { address: "shekyl1second", amount: "2000000000" };
      },
    });
    const u = user();
    render(<Send />);
    const field = screen.getByPlaceholderText("shekyl1...");
    await u.click(field);
    await u.paste("shekyl:shekyl1first?amount=1000000000");
    await u.clear(field);
    await u.paste("shekyl:shekyl1second?amount=2000000000");
    await waitFor(() => expect(field).toHaveValue("shekyl1second"));
    resolveFirst({ address: "shekyl1first", amount: "1000000000" });
    await new Promise((r) => setTimeout(r, 20));
    expect(field).toHaveValue("shekyl1second");
    expect(screen.getByPlaceholderText("0.0000")).toHaveValue("2.000000000");
  });

  it("a malformed payment link is refused and the field keeps what was typed", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      parse_uri: () => {
        throw "invalid payment URI: missing or empty address in payment URI";
      },
    });
    const u = user();
    render(<Send />);
    await u.click(screen.getByPlaceholderText("shekyl1..."));
    await u.paste("shekyl:");
    expect(await screen.findByRole("alert")).toHaveTextContent(/invalid payment URI/);
    expect(screen.getByPlaceholderText("shekyl1...")).toHaveValue("shekyl:");
    expect(screen.queryByTestId("link-notice")).not.toBeInTheDocument();
  });

  it("a retained reservation is never discarded by the page", async () => {
    route({
      get_default_fee_priority: () => QUOTE,
      build_pending_tx: () => BUILT,
      submit_pending_tx: () => {
        throw AMBIGUOUS;
      },
    });
    const u = user();
    const view = render(<Send />);
    await reachReview(u);
    await confirm(u);
    expect(await screen.findByRole("alert")).toHaveTextContent(/check Transactions/);
    view.unmount();
    await new Promise((r) => setTimeout(r, 50));
    expect(calls("discard_pending_tx")).toHaveLength(0);
  });
});
