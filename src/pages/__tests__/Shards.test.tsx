import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ShardPickerProvider } from "../../context/ShardPickerContext";
import Shards from "../Shards";
import type { ShardCoverageList, ShardCoverageRow } from "../../types/shards";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

const SAMPLE_ROW: ShardCoverageRow = {
  shard_id: 2,
  bonded_count: 1,
  served_count: 0,
  freeze_height: 1000,
  join_scarcity_micro: 900_000,
  expected_profit_atomic: 5_000_000_000,
};

const SAMPLE_LIST: ShardCoverageList = {
  as_of_height: 50_000,
  leaf_count: 77_976,
  frozen_count: 1,
  settled_epoch: 1,
  budget_atomic: 1,
  sigma_work_milli: 1,
  profit_estimate_available: true,
  shards: [SAMPLE_ROW],
};

function mockCoverage(list: ShardCoverageList) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "list_shards") return list;
    if (cmd === "get_shard_render") {
      throw new Error("lazy PNG must not fetch on mount");
    }
    return null;
  });
}

function renderShards() {
  return render(
    <MemoryRouter>
      <ShardPickerProvider>
        <Shards />
      </ShardPickerProvider>
    </MemoryRouter>,
  );
}

describe("Shards", () => {
  it("renders the heading and profit-led copy, not a fixture banner", async () => {
    mockCoverage({ ...SAMPLE_LIST, frozen_count: 0, shards: [] });
    renderShards();
    expect(screen.getByText("Shards")).toBeInTheDocument();
    expect(
      screen.getByText(/the network does not assign them/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/Pre-archival preview/)).not.toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText(/No frozen archives yet/)).toBeInTheDocument();
    });
  });

  it("lists profit, not regime or tier fields, and does not fetch PNGs on mount", async () => {
    mockCoverage(SAMPLE_LIST);
    renderShards();
    await waitFor(() => {
      expect(screen.getByText("Archive #2")).toBeInTheDocument();
    });
    expect(screen.getByText("5.000000 SKL / epoch")).toBeInTheDocument();
    expect(screen.queryByText("genesis")).not.toBeInTheDocument();
    expect(screen.queryByText(/Tier distribution/)).not.toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.every((c) => c[0] !== "get_shard_render"),
    ).toBe(true);
  });

  it("toggles a card into session selection", async () => {
    const user = userEvent.setup();
    mockCoverage(SAMPLE_LIST);
    renderShards();
    const card = await screen.findByRole("button", { name: /Archive #2/i });
    await user.click(card);
    expect(screen.getByText("Selected")).toBeInTheDocument();
    expect(screen.getByText(/1 of 4096 archives selected/)).toBeInTheDocument();
  });

  it("surfaces a backend error with retry and never substitutes fixtures", async () => {
    vi.mocked(invoke).mockRejectedValue("boom");
    renderShards();
    await waitFor(() => {
      expect(screen.getByText(/Could not load archive coverage/)).toBeInTheDocument();
      expect(screen.getByText(/boom/)).toBeInTheDocument();
    });
    expect(screen.getByText("Retry")).toBeInTheDocument();
    expect(screen.queryByText(/Genesis regime/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Pre-archival preview/)).not.toBeInTheDocument();
  });

  it("keeps the list when one lazy render fails", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_shards") return SAMPLE_LIST;
      if (cmd === "get_shard_render") {
        throw new Error("could not retrieve this archive");
      }
      return null;
    });
    renderShards();
    expect(await screen.findByText("Archive #2")).toBeInTheDocument();
    expect(screen.getByText("5.000000 SKL / epoch")).toBeInTheDocument();
  });
});
