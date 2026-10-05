import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PaymentProof from "../PaymentProof";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("PaymentProof", () => {
  it("shows a proof that does not check out as a fault", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "check_tx_proof") return Promise.resolve({ valid: false });
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<PaymentProof />);
    await user.type(screen.getByLabelText("Transaction id"), "ab");
    await user.type(screen.getByLabelText("Address"), "addr");
    await user.type(screen.getByLabelText("Payment proof"), "proof");
    await user.click(screen.getByRole("button", { name: "Check payment" }));
    const notice = await screen.findByText("This payment proof does not check out.");
    expect(notice).toHaveClass("text-red-300");
    expect(notice).not.toHaveClass("text-emerald-200");
  });

  it("drops a passing check when an input changes", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "check_tx_proof") {
        return Promise.resolve({ valid: true, received: "1000000000", confirmations: 1 });
      }
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<PaymentProof />);
    await user.type(screen.getByLabelText("Transaction id"), "ab");
    await user.type(screen.getByLabelText("Address"), "addr");
    await user.type(screen.getByLabelText("Payment proof"), "proof");
    await user.click(screen.getByRole("button", { name: "Check payment" }));
    expect(await screen.findByText(/checks out/)).toHaveClass("text-emerald-200");
    await user.type(screen.getByLabelText("Transaction id"), "c");
    expect(screen.queryByText(/checks out/)).not.toBeInTheDocument();
  });

  it("keeps a proof the person typed when an older prove finishes", async () => {
    let finish: (value: { proof: string; direction: string }) => void = () => {};
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_tx_proof") {
        return new Promise((resolve) => {
          finish = resolve;
        });
      }
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<PaymentProof />);
    await user.type(screen.getByLabelText("Transaction id"), "ab");
    await user.type(screen.getByLabelText("Address"), "addr");
    await user.click(screen.getByRole("button", { name: "Prove payment" }));
    await user.type(screen.getByLabelText("Payment proof"), "typed");
    await act(async () => {
      finish({ proof: "from-server", direction: "OUTBOUND" });
    });
    expect(screen.getByLabelText("Payment proof")).toHaveValue("typed");
    expect(screen.queryByText(/Payment proof created/)).not.toBeInTheDocument();
  });
});
