import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Receive from "../Receive";
import type { PaymentRequest } from "../../types/receiving";

const MOCK_ADDRESS = "SKL1mock_account0_subaddr0...placeholder";
const PENDING_URI = `shekyl:${MOCK_ADDRESS}?amount=1500000000&label=Invoice%201042&rid=42&expiry=1700003600`;
const PENDING: PaymentRequest = {
  id: "42",
  label: "Invoice 1042",
  amount: "1500000000",
  created_at: 1_700_000_000,
  expiry: 1_700_003_600,
  state: "PENDING",
  uri: PENDING_URI,
};
const PAID: PaymentRequest = {
  id: "43",
  label: "",
  amount: "9007199254740993",
  created_at: 1_700_000_100,
  state: "MATCHED",
  matched_tx_hash: "ab".repeat(32),
  matched_output_index: 0,
  uri: `shekyl:${MOCK_ADDRESS}?amount=9007199254740993&rid=43`,
};

type Handler = (args?: Record<string, unknown>) => unknown;
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

const BASE: Record<string, Handler> = {
  get_primary_address: () => ({ address: MOCK_ADDRESS }),
  list_payment_requests: () => ({ payment_requests: [] }),
};

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  route(BASE);
});

describe("Receive page", () => {
  it("renders the page title", async () => {
    render(<Receive />);
    expect(screen.getByText("Receive SKL")).toBeInTheDocument();
    // The list's read settles after the title paints. Wait for it so the
    // update is not left hanging when the test ends.
    expect(await screen.findByText("No payment requests yet.")).toBeInTheDocument();
  });

  it("displays the receiving address from the backend", async () => {
    render(<Receive />);
    await waitFor(() => {
      expect(screen.getByText(MOCK_ADDRESS)).toBeInTheDocument();
    });
  });

  it("copies the address to the clipboard on button click", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, writable: true, configurable: true });
    render(<Receive />);
    await waitFor(() => {
      expect(screen.getByText(MOCK_ADDRESS)).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTitle("Copy full address"));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(MOCK_ADDRESS);
    });
  });

  it("says so when the clipboard refuses, instead of pretending it copied", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, writable: true, configurable: true });
    render(<Receive />);
    await waitFor(() => {
      expect(screen.getByText(MOCK_ADDRESS)).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTitle("Copy full address"));
    expect(await screen.findByRole("alert")).toHaveTextContent(/copy failed/i);
  });

  it("shows 'No wallet open' when address is empty, and no request panels", () => {
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
    render(<Receive />);
    expect(screen.getAllByText("No wallet open").length).toBeGreaterThan(0);
    expect(screen.queryByRole("form", { name: /request a payment/i })).not.toBeInTheDocument();
  });
});

describe("Payment requests", () => {
  it("creates a request with the contract's parameters and shows its link", async () => {
    const u = userEvent.setup();
    let created: Record<string, unknown> | undefined;
    route({
      ...BASE,
      create_payment_request: (args) => {
        created = args;
        return { id: "42", uri: `shekyl:${MOCK_ADDRESS}?amount=1500000000&label=Invoice%201042&rid=42` };
      },
    });
    render(<Receive />);
    await screen.findByRole("form", { name: /request a payment/i });
    await u.type(screen.getByLabelText(/amount \(skl\)/i), "1.5");
    await u.type(screen.getByLabelText(/^label/i), "Invoice 1042");
    await u.selectOptions(screen.getByLabelText(/expires/i), "24h");
    const before = Math.floor(Date.now() / 1000);
    await u.click(screen.getByRole("button", { name: /create payment link/i }));
    await screen.findByTestId("payment-link");
    expect(created).toMatchObject({ label: "Invoice 1042", amount: "1500000000" });
    const expiry = created?.expiry as number;
    expect(expiry).toBeGreaterThanOrEqual(before + 24 * 3600);
    expect(expiry).toBeLessThanOrEqual(before + 24 * 3600 + 5);
    expect(screen.getByText(/rid=42/)).toBeInTheDocument();
    // The list refetches so the new request appears without a reload.
    expect(calls("list_payment_requests").length).toBeGreaterThanOrEqual(2);
  });

  it("refuses an amount finer than an atomic unit before any call", async () => {
    const u = userEvent.setup();
    render(<Receive />);
    await screen.findByRole("form", { name: /request a payment/i });
    await u.type(screen.getByLabelText(/amount \(skl\)/i), "0.0000000001");
    await u.click(screen.getByRole("button", { name: /create payment link/i }));
    expect(calls("create_payment_request")).toHaveLength(0);
  });

  it("lists requests with their state, exact amounts and a link for pending ones", async () => {
    const u = userEvent.setup();
    let listed: Record<string, unknown> | undefined;
    route({
      ...BASE,
      list_payment_requests: (args) => {
        listed = args;
        return { payment_requests: [PENDING, PAID] };
      },
    });
    render(<Receive />);
    expect(await screen.findByText("Invoice 1042")).toBeInTheDocument();
    expect(listed).toEqual({ filter: "ALL" });
    const pendingRow = screen.getByText("Invoice 1042").closest("li")!;
    const paidRow = screen.getByText("Request 43").closest("li")!;
    expect(within(pendingRow).getByText("Awaiting payment")).toBeInTheDocument();
    expect(within(pendingRow).getByText("1.500000000 SKL")).toBeInTheDocument();
    expect(within(paidRow).getByText("Paid")).toBeInTheDocument();
    expect(within(paidRow).getByText("9007199.254740993 SKL")).toBeInTheDocument();

    await u.click(screen.getByRole("button", { name: /show payment link for invoice 1042/i }));
    expect(await screen.findByTestId("payment-link")).toHaveTextContent(PENDING_URI);
    expect(calls("make_uri")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: /show payment link for request 43/i })).not.toBeInTheDocument();
  });

  it("a failed list is a fault, not a loading line that never ends", async () => {
    route({
      ...BASE,
      list_payment_requests: () => {
        throw "could not read payment requests";
      },
    });
    render(<Receive />);
    expect(await screen.findByRole("alert")).toHaveTextContent("could not read payment requests");
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
    expect(screen.queryByText("No payment requests yet.")).not.toBeInTheDocument();
  });

  it("a slow list for the previous filter cannot overwrite the one on screen", async () => {
    let resolveAll: (value: unknown) => void = () => {};
    route({
      ...BASE,
      list_payment_requests: (args) => {
        if (args?.filter === "ALL") return new Promise((resolve) => (resolveAll = resolve));
        return { payment_requests: [{ ...PENDING, label: "From pending" }] };
      },
    });
    const u = userEvent.setup();
    render(<Receive />);
    await u.click(await screen.findByRole("tab", { name: "Pending" }));
    expect(await screen.findByText("From pending")).toBeInTheDocument();
    resolveAll({ payment_requests: [{ ...PENDING, label: "From all" }] });
    await new Promise((r) => setTimeout(r, 30));
    expect(screen.queryByText("From all")).not.toBeInTheDocument();
    expect(screen.getByText("From pending")).toBeInTheDocument();
  });

  it("filters by the contract's names", async () => {
    const u = userEvent.setup();
    const filters: unknown[] = [];
    route({
      ...BASE,
      list_payment_requests: (args) => {
        filters.push(args?.filter);
        return { payment_requests: [] };
      },
    });
    render(<Receive />);
    await screen.findByRole("tab", { name: "Pending" });
    await u.click(screen.getByRole("tab", { name: "Pending" }));
    await u.click(screen.getByRole("tab", { name: "Paid" }));
    await waitFor(() => expect(filters).toEqual(["ALL", "PENDING", "MATCHED"]));
  });
});
