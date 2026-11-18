import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import PreWalletShell from "../PreWalletShell";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(null);
});

describe("PreWalletShell", () => {
  /**
   * The page sits in a region of its own that scrolls, under the node's
   * bar, in a window that does not (finding on #71: a long notice pushed
   * a page's controls out of reach). Red edit: render the page beside the
   * bar with no region of its own.
   */
  it("gives the page a scrolling region of its own under the node's bar", () => {
    render(
      <PreWalletShell>
        <button>Open wallet</button>
      </PreWalletShell>,
    );
    const page = screen.getByTestId("pre-wallet-page");
    expect(page).toContainElement(screen.getByRole("button", { name: "Open wallet" }));
    expect(page).toHaveClass("overflow-y-auto", "min-h-0", "flex-1");
    expect(page.parentElement).toHaveClass("h-screen", "flex-col", "overflow-hidden");
    expect(page).not.toContainElement(screen.getByText("Network"));
  });
});
