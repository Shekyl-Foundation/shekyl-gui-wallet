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

  it("asks for the new password twice and refuses a short one", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<WalletCare />);
    await user.type(screen.getByLabelText("Current password"), "current-password");
    await user.type(screen.getByLabelText("New password"), "short");
    await user.type(screen.getByLabelText("Confirm new password"), "short");
    expect(screen.getByRole("button", { name: "Change password" })).toBeDisabled();
    await user.clear(screen.getByLabelText("New password"));
    await user.type(screen.getByLabelText("New password"), "long-enough");
    expect(screen.getByText("The new passwords do not match.")).toBeInTheDocument();
    await user.clear(screen.getByLabelText("Confirm new password"));
    await user.type(screen.getByLabelText("Confirm new password"), "long-enough");
    await user.click(screen.getByRole("button", { name: "Change password" }));
    expect(await screen.findByText("Password changed.")).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["change_password", { oldPassword: "current-password", newPassword: "long-enough" }],
    ]);
  });
});
