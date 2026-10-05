import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import ReserveProof from "../ReserveProof";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("ReserveProof", () => {
  it("drops a passing check when the proof changes", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "check_reserve_proof") {
        return Promise.resolve({ valid: true, total: "1000000000", spent: "0", output_count: 1 });
      }
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<ReserveProof />);
    await user.type(screen.getByLabelText(/Address/), "addr");
    await user.type(screen.getByLabelText("Reserve proof"), "proof");
    await user.click(screen.getByRole("button", { name: "Check reserve" }));
    expect(await screen.findByText(/checks out/)).toHaveClass("text-emerald-200");
    await user.type(screen.getByLabelText("Reserve proof"), "x");
    expect(screen.queryByText(/checks out/)).not.toBeInTheDocument();
  });
});
