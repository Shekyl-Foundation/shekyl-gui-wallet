import { render, screen } from "@testing-library/react";
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
});
