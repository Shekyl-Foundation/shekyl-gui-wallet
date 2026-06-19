import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Staking from "../Staking";
import type { StakeView } from "../../types/staking";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function stake(over: Partial<StakeView>): StakeView {
  return {
    tx_hash: "ab".repeat(32),
    global_output_index: 1,
    amount: 5_000_000_000,
    tier: 0,
    stake_height: 100,
    stake_lock_until: 200,
    blocks_until_mature: 0,
    matured: true,
    pending_unstake: false,
    accrued_claimable: 0,
    estimated_yield_to_maturity: 0,
    ...over,
  };
}

function mockStakes(views: StakeView[]) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_stake_views") return views;
    if (cmd === "get_tier_yields") return [];
    // The page embeds <ShardIdentityPreview>, which loads fixtures on mount.
    if (cmd === "list_shard_preview_fixtures") return [];
    return null;
  });
}

function renderStaking() {
  return render(
    <MemoryRouter>
      <DaemonProvider>
        <Staking />
      </DaemonProvider>
    </MemoryRouter>,
  );
}

describe("Staking", () => {
  it("renders the page heading", () => {
    mockStakes([]);
    renderStaking();
    expect(screen.getByText("Staking")).toBeInTheDocument();
  });

  it("shows a matured stake as unlocked with an Unstake button", async () => {
    mockStakes([stake({ global_output_index: 1, matured: true })]);
    renderStaking();
    await waitFor(() => {
      expect(screen.getByText("Unlocked")).toBeInTheDocument();
    });
    expect(screen.getByText("Unstake")).toBeInTheDocument();
  });

  it("shows a maturity countdown for a locked stake and no Unstake button", async () => {
    mockStakes([
      stake({
        global_output_index: 2,
        matured: false,
        blocks_until_mature: 1440,
      }),
    ]);
    renderStaking();
    await waitFor(() => {
      expect(screen.getByText(/1,440 blk/)).toBeInTheDocument();
    });
    expect(screen.queryByText("Unstake")).not.toBeInTheDocument();
  });

  it("badges an in-flight unstake and hides the Unstake button", async () => {
    mockStakes([
      stake({ global_output_index: 3, matured: true, pending_unstake: true }),
    ]);
    renderStaking();
    await waitFor(() => {
      expect(screen.getByText("Unstaking…")).toBeInTheDocument();
    });
    expect(screen.queryByText("Unstake")).not.toBeInTheDocument();
  });

  it("offers Claim when rewards have accrued", async () => {
    mockStakes([
      stake({ global_output_index: 4, accrued_claimable: 1_000_000_000 }),
    ]);
    renderStaking();
    await waitFor(() => {
      expect(screen.getByText("Claim")).toBeInTheDocument();
    });
  });
});
