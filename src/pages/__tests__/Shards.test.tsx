import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Shards from "../Shards";
import type { ShardSummary } from "../../types/shards";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

const SAMPLE: ShardSummary = {
  label: "Genesis regime",
  aggregate: {
    shard_id: 0,
    shard_hash: "82a866d2e033b952a133a0cb4595fa736d23584b20a88df66dbb5e4a8eedb750",
    block_count: 10_000,
    tx_count: 516,
    output_count: 11_324,
    coinbase_output_count: 10_000,
    time_range_seconds: 1_196_437,
    coinbase_ratio: 0.88,
    value_log_mean: 16.1,
    value_log_variance: 30.9,
    stake_events_created: 3,
    stake_events_claimed: 0,
    tier_distribution: [0, 0, 3],
    dominant_regime: "genesis",
  },
};

function mockShards(list: ShardSummary[]) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "list_shards") return list;
    if (cmd === "get_shard_render") {
      return { png_base64: "AAAA", recipe: {}, cache_key: "k", shard_id: 0 };
    }
    return null;
  });
}

function renderShards() {
  return render(
    <MemoryRouter>
      <Shards />
    </MemoryRouter>,
  );
}

describe("Shards", () => {
  it("renders the page heading and pre-archival note", () => {
    mockShards([]);
    renderShards();
    expect(screen.getByText("Shards")).toBeInTheDocument();
    expect(screen.getByText(/Pre-archival preview/)).toBeInTheDocument();
  });

  it("lists shards returned by the backend", async () => {
    mockShards([SAMPLE]);
    renderShards();
    await waitFor(() => {
      expect(screen.getByText("Genesis regime")).toBeInTheDocument();
    });
    expect(screen.getByText("Shard #0")).toBeInTheDocument();
    expect(screen.getByText("genesis")).toBeInTheDocument();
    // Tier distribution short/med/long.
    expect(screen.getByText("0 / 0 / 3")).toBeInTheDocument();
  });

  it("surfaces a backend error with retry", async () => {
    vi.mocked(invoke).mockRejectedValue("boom");
    renderShards();
    await waitFor(() => {
      expect(screen.getByText("boom")).toBeInTheDocument();
    });
    expect(screen.getByText("Retry")).toBeInTheDocument();
  });
});
