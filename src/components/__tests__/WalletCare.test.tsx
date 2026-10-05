import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import WalletCare from "../WalletCare";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("WalletCare", () => {
  it("does not rebuild history until the person confirms", async () => {
    vi.mocked(invoke).mockResolvedValue({
      blocks_processed: 1,
      transfers_detected: 0,
      synced_height: 4,
    });
    const user = userEvent.setup();
    render(<WalletCare />);
    expect(invoke).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Rebuild history" }));
    expect(invoke).not.toHaveBeenCalled();
    expect(screen.getByText(/re-reads the chain from the start/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Keep history" }));
    expect(invoke).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Rebuild history" }));
    await user.click(screen.getByRole("button", { name: "Rebuild now" }));
    expect(await screen.findByText(/Rescan finished/)).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.map(([cmd]) => cmd)).toEqual(["rescan_blockchain"]);
  });
});
