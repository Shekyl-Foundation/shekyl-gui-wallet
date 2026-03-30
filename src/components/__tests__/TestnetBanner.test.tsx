import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import TestnetBanner from "../TestnetBanner";

vi.mock("../../context/useDaemon", () => ({
  useDaemon: vi.fn(),
}));

import { useDaemon } from "../../context/useDaemon";

const baseHealth = {
  height: 100,
  target_height: 100,
  top_block_hash: "abc",
  difficulty: 1000,
  tx_count: 50,
  tx_pool_size: 0,
  database_size: 0,
  version: "0.1.0",
  synchronized: true,
  already_generated_coins: "0",
  release_multiplier: 1_000_000,
  burn_pct: 50_000,
  stake_ratio: 200_000,
  total_burned: 0,
  staker_pool_balance: 0,
  staker_emission_share_effective: 100_000,
  emission_era: "Foundation",
  last_block_reward: 0,
  last_block_timestamp: 0,
  last_block_hash: "",
  last_block_size: 0,
  total_staked: 0,
  tier_0_lock_blocks: 0,
  tier_1_lock_blocks: 0,
  tier_2_lock_blocks: 0,
  network: "mainnet",
};

describe("TestnetBanner", () => {
  it("renders nothing on mainnet", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: { ...baseHealth, network: "mainnet" },
      pqc: null,
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
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<TestnetBanner />);

    expect(screen.getByText(/StageNet/)).toBeInTheDocument();
  });
});
