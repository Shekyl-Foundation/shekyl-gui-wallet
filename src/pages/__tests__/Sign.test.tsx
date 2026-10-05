import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Sign from "../Sign";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("Sign", () => {
  it("shows a failed verification as a fault, not a success", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "verify_message") {
        return Promise.reject({ message: "signature does not verify" });
      }
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<Sign />);
    await user.type(screen.getByLabelText("Message"), "hello");
    await user.type(screen.getByLabelText(/Address/), "addr");
    await user.type(screen.getByLabelText("Signature"), "sig");
    await user.click(screen.getByRole("button", { name: "Verify" }));
    const notice = await screen.findByText("signature does not verify");
    expect(notice).toHaveClass("text-red-300");
    expect(notice).not.toHaveClass("text-emerald-200");
  });

  it("drops a match when the signature changes, and keeps a signature when only the address changes", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "verify_message") return Promise.resolve(null);
      if (cmd === "sign_message") return Promise.resolve({ signature: "sig" });
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<Sign />);
    await user.type(screen.getByLabelText("Message"), "hello");
    await user.click(screen.getByRole("button", { name: "Sign" }));
    expect(await screen.findByText(/Share the message/)).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Address/), "addr");
    expect(screen.getByText(/Share the message/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Signature"), "x");
    expect(screen.queryByText(/Share the message/)).not.toBeInTheDocument();
  });
});
