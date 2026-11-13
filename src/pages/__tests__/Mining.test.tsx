import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
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

/** A 4.0 `get_mining_status` answer: the node's `mine_status`. */
function miningStatus(active: boolean) {
  return {
    active,
    hash_rate: active ? 1200 : null,
    threads: active ? 4 : 2,
    address: active ? "SKL1test..." : null,
    block_spacing: 120,
    height: 10,
    block_reward: "5000000000",
    difficulty: "100000",
    cause: null,
  };
}

function mockMiningStatus(active: boolean) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_mining_status") return miningStatus(active);
    return null;
  });
}

describe("Mining", () => {
  it("renders the page heading", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining")).toBeInTheDocument();
  });

  it("names no lock window before the node is read", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining and Privacy")).toBeInTheDocument();
    expect(screen.getByText(/wait a fixed number of blocks/)).toBeInTheDocument();
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

  it("renders mining controls without an address to type", () => {
    mockMiningStatus(false);
    renderMining();
    expect(screen.getByText("Mining Controls")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("SKL1...")).not.toBeInTheDocument();
    expect(screen.queryByText(/Background mining/)).not.toBeInTheDocument();
    expect(screen.getByText("Start Mining")).toBeInTheDocument();
  });

  it("starts the node's miner with the thread count alone", async () => {
    mockMiningStatus(false);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("Start Mining")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText("Start Mining"));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("start_mining_cmd", { threads: 2 });
    });
  });

  it("names the address the node's miner pays", async () => {
    mockMiningStatus(true);
    renderMining();
    expect(await screen.findByText("SKL1test...")).toBeInTheDocument();
  });

  it("says when the node has no mining address", async () => {
    mockMiningStatus(false);
    renderMining();
    expect(await screen.findByText("No mining address is set on this node.")).toBeInTheDocument();
  });

  it("shows the node's sentence when it refuses the miner's status", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_mining_status") {
        throw "This node does not let this wallet see or change mining.";
      }
      return null;
    });
    renderMining();
    expect(
      await screen.findByText("This node does not let this wallet see or change mining."),
    ).toBeInTheDocument();
    // A refused read says nothing about the node's address.
    expect(screen.queryByText("No mining address is set on this node.")).toBeNull();
  });

  it("shows why no job is posted", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_mining_status") {
        return { ...miningStatus(false), cause: "the node is still syncing" };
      }
      return null;
    });
    renderMining();
    expect(await screen.findByText("the node is still syncing")).toBeInTheDocument();
  });

  it("takes the lock window and spacing from core, and names no ring", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_wallet_status") return { connected: true };
      if (cmd === "get_chain_health") {
        return { coinbase_lock_window: 7, block_spacing: 60, difficulty: "0" };
      }
      if (cmd === "get_mining_status") return miningStatus(false);
      return null;
    });
    renderMining();
    expect(
      await screen.findByText(/Mining rewards can be spent after 7 blocks \(about 7 minutes\)\./),
    ).toBeInTheDocument();
    expect(screen.getByText(/Shekyl has no rings/)).toBeInTheDocument();
    expect(screen.queryByText(/mix ring/)).toBeNull();
  });

  it("shows the hash rate from the node's window", async () => {
    mockMiningStatus(true);
    renderMining();
    expect(await screen.findAllByText("1.20 KH/s")).toHaveLength(2);
  });

  it("renders the algorithm name", async () => {
    mockMiningStatus(false);
    renderMining();
    await waitFor(() => {
      expect(screen.getByText("RandomX")).toBeInTheDocument();
    });
  });
});
