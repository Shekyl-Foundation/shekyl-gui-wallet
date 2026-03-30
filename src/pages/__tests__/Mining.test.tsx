import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Mining from "../Mining";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function renderMining() {
  return render(
    <MemoryRouter>
      <DaemonProvider>
        <Mining />
      </DaemonProvider>
    </MemoryRouter>,
  );
}

function mockMiningStatus(active: boolean) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_mining_status") {
      return {
        active,
        speed: active ? 1200 : 0,
        threads_count: active ? 4 : 0,
        address: active ? "SKL1test..." : "",
        pow_algorithm: "RandomX",
        is_background_mining_enabled: false,
        block_target: 120,
        block_reward: 5_000_000_000,
        difficulty: 100000,
      };
    }
    return null;
  });
}

describe("Mining", () => {
  it("renders the page heading", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining")).toBeInTheDocument();
  });

  it("shows privacy note about mining rewards", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining and Privacy")).toBeInTheDocument();
    expect(screen.getByText(/locked for 60 blocks/)).toBeInTheDocument();
  });

  it("shows idle status when not mining", async () => {
    mockMiningStatus(false);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("Idle")).toBeInTheDocument();
    });
  });

  it("shows mining status when active", async () => {
    mockMiningStatus(true);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("Mining")).toBeInTheDocument();
    });
  });

  it("renders mining controls", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining Controls")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("SKL1...")).toBeInTheDocument();
    expect(screen.getByText("Start Mining")).toBeInTheDocument();
  });

  it("shows error when starting without address", async () => {
    mockMiningStatus(false);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("Start Mining")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("Start Mining"));
    await waitFor(() => {
      expect(screen.getByText("Enter a miner address")).toBeInTheDocument();
    });
  });

  it("renders the algorithm name", async () => {
    mockMiningStatus(false);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("RandomX")).toBeInTheDocument();
    });
  });

  it("renders the daemon requirement note", () => {
    mockMiningStatus(false);
    renderMining();
    expect(
      screen.getByText(/unrestricted daemon/),
    ).toBeInTheDocument();
  });
});
