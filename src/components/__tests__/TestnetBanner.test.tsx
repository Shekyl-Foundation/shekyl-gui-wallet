import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import TestnetBanner from "../TestnetBanner";
import type { ChainHealth } from "../../types/daemon";

vi.mock("../../context/useDaemon", () => ({
  useDaemon: vi.fn(),
}));

import { useDaemon } from "../../context/useDaemon";

const baseHealth: ChainHealth = {
  height: 0,
  target_height: null,
  hash: "",
  difficulty: "0",
  tx_count: 0,
  txpool_count: 0,
  synchronized: true,
  emission: "0",
  emission_multiplier: 1_000_000,
  burn: 0,
  burn_total: "0",
  stake_share: 0,
  block_spacing: 120,
  network: "mainnet",
  last_block: null,
  curve_tree: null,
};

describe("TestnetBanner", () => {
  it("renders nothing on mainnet", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: { ...baseHealth, network: "mainnet" },
      pqc: null,
      security: null,
      status: null,
      connected: true,
      hasEverConnected: true,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    const { container } = render(<TestnetBanner />);
    expect(container.innerHTML).toBe("");
  });

  it("renders testnet warning with faucet link", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: { ...baseHealth, network: "testnet" },
      pqc: null,
      security: null,
      status: null,
      connected: true,
      hasEverConnected: true,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<TestnetBanner />);

    expect(screen.getByText(/TestNet Active/)).toBeInTheDocument();
    expect(screen.getByText("Get test SKL")).toBeInTheDocument();
    expect(screen.getByText("Get test SKL").closest("a")?.href).toBe(
      "https://faucet.shekyl.org/",
    );
  });

  it("renders stagenet banner", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: { ...baseHealth, network: "stagenet" },
      pqc: null,
      security: null,
      status: null,
      connected: true,
      hasEverConnected: true,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<TestnetBanner />);

    expect(screen.getByText(/StageNet/)).toBeInTheDocument();
  });
});
