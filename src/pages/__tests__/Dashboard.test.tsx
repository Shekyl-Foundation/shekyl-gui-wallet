import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Dashboard from "../Dashboard";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function renderDashboard() {
  return render(
    <MemoryRouter>
      <DaemonProvider>
        <Dashboard />
      </DaemonProvider>
    </MemoryRouter>,
  );
}

describe("Dashboard", () => {
  it("renders the heading and quick action links", () => {
    vi.mocked(invoke).mockRejectedValue(new Error("no daemon"));

    renderDashboard();

    expect(screen.getByText("Dashboard")).toBeInTheDocument();
    expect(screen.getByText("Send")).toBeInTheDocument();
    expect(screen.getByText("Receive")).toBeInTheDocument();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(screen.getByText("History")).toBeInTheDocument();
  });

  it("renders the BalanceCard", () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_balance") {
        return { total: 0, unlocked: 0, staked: 0 };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    renderDashboard();

    expect(screen.getByText("Total Balance")).toBeInTheDocument();
  });
});
