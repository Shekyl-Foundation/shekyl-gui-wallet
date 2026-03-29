import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Dashboard from "../Dashboard";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function renderDashboard() {
  return render(
    <MemoryRouter>
      <Dashboard />
    </MemoryRouter>,
  );
}

describe("Dashboard", () => {
  it("shows the welcome screen when no wallet is open", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_wallet_status") {
        return { connected: false, wallet_open: false };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Welcome to Shekyl Wallet")).toBeInTheDocument();
    });

    expect(screen.getByText("Create Wallet")).toBeInTheDocument();
    expect(screen.getByText("Open Wallet")).toBeInTheDocument();
  });

  it("shows the dashboard with quick actions when wallet is open", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_wallet_status") {
        return { connected: true, wallet_open: true };
      }
      if (cmd === "get_balance") {
        return { total: 1_000_000_000_000, unlocked: 1_000_000_000_000, staked: 0 };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    renderDashboard();

    await waitFor(() => {
      expect(screen.getByText("Dashboard")).toBeInTheDocument();
    });

    expect(screen.getByText("Send")).toBeInTheDocument();
    expect(screen.getByText("Receive")).toBeInTheDocument();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(screen.getByText("History")).toBeInTheDocument();
  });
});
