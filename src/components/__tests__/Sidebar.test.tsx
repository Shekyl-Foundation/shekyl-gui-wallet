import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Sidebar from "../Sidebar";
import { resetFeatureFlagsForTests } from "../../features";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  resetFeatureFlagsForTests();
});

function renderSidebar(initialRoute = "/") {
  return render(
    <MemoryRouter initialEntries={[initialRoute]}>
      <Sidebar />
    </MemoryRouter>,
  );
}

describe("Sidebar", () => {
  it("renders all navigation links", () => {
    renderSidebar();

    expect(screen.getByText("Dashboard")).toBeInTheDocument();
    expect(screen.getByText("Send")).toBeInTheDocument();
    expect(screen.getByText("Receive")).toBeInTheDocument();
    expect(screen.getByText("Mining")).toBeInTheDocument();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(screen.getByText("Transactions")).toBeInTheDocument();
    expect(screen.getByText("Chain Health")).toBeInTheDocument();
    expect(screen.getByText("Help")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("hides Multisig by default — the gate fails closed when Rust does not answer", async () => {
    // The global mock rejects `invoke`; the flags must stay at their default.
    renderSidebar();
    expect(screen.queryByText("Multisig")).not.toBeInTheDocument();
    await screen.findByText("Dashboard");
    expect(screen.queryByText("Multisig")).not.toBeInTheDocument();
  });

  it("shows Multisig only when the compiled feature set says so", async () => {
    vi.mocked(invoke).mockImplementation((cmd) =>
      cmd === "get_feature_flags"
        ? Promise.resolve({ multisig: true })
        : Promise.reject(new Error(`unexpected invoke ${String(cmd)}`)),
    );
    renderSidebar();
    expect(await screen.findByText("Multisig")).toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_feature_flags");
  });

  it("renders the Shekyl branding", () => {
    renderSidebar();
    expect(screen.getByText("Shekyl")).toBeInTheDocument();
    expect(screen.getByAltText("Shekyl")).toBeInTheDocument();
  });

  it("renders the version string", () => {
    renderSidebar();
    expect(screen.getByText(/Shekyl Wallet v/)).toBeInTheDocument();
  });

  it("highlights the active route", () => {
    renderSidebar("/send");
    const sendLink = screen.getByText("Send").closest("a")!;
    expect(sendLink.className).toContain("text-gold-400");
  });
});
