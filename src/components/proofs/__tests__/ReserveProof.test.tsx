import { act, render, screen } from "@testing-library/react";
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

  it("keeps a proof the person typed when an older prove finishes", async () => {
    let finish: (value: { proof: string; total: string; output_count: number }) => void = () => {};
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_reserve_proof") {
        return new Promise((resolve) => {
          finish = resolve;
        });
      }
      return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    });
    const user = userEvent.setup();
    render(<ReserveProof />);
    await user.click(screen.getByRole("button", { name: "Prove reserve" }));
    await user.type(screen.getByLabelText("Reserve proof"), "typed");
    await act(async () => {
      finish({ proof: "from-server", total: "1000000000", output_count: 1 });
    });
    expect(screen.getByLabelText("Reserve proof")).toHaveValue("typed");
    expect(screen.queryByText(/Reserve proof created/)).not.toBeInTheDocument();
  });
});
