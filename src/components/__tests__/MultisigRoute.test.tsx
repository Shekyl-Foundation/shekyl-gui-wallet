import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import MultisigRoute from "../multisig/MultisigRoute";
import { resetFeatureFlagsForTests } from "../../features";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  resetFeatureFlagsForTests();
});

describe("MultisigRoute", () => {
  it("does not mount the page when Rust does not enable the feature", async () => {
    render(<MultisigRoute />);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_feature_flags");
    });
    expect(screen.queryByRole("heading", { name: "Multisig" })).not.toBeInTheDocument();
    const commands = vi.mocked(invoke).mock.calls.map(([cmd]) => cmd);
    expect(commands).not.toContain("get_multisig_info");
  });

  it("mounts the page only after Rust reports the feature", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_feature_flags") return Promise.resolve({ multisig: true });
      return Promise.reject(new Error(`unexpected ${String(cmd)}`));
    });
    render(<MultisigRoute />);
    expect(await screen.findByRole("heading", { name: "Multisig" })).toBeInTheDocument();
  });
});
