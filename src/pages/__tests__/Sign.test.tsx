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
});
