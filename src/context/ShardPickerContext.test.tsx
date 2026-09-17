import { useEffect, type MutableRefObject } from "react";
import { act, render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { ShardPickerProvider } from "./ShardPickerContext";
import { useShardPicker } from "./useShardPicker";
import type { ShardPickerState } from "./shardPickerState";
import { MAX_HOLDINGS_SHARDS, type ShardCoverageRow } from "../types/shards";

function row(shard_id: number, expected_profit_atomic: string): ShardCoverageRow {
  return {
    shard_id,
    bonded_count: 0,
    served_count: 0,
    freeze_height: 1,
    join_scarcity_micro: 1,
    expected_profit_atomic,
  };
}

function ApiRef({
  apiRef,
}: {
  apiRef: MutableRefObject<ShardPickerState | null>;
}) {
  const api = useShardPicker();
  useEffect(() => {
    apiRef.current = api;
  });
  return null;
}

function mountPicker() {
  const apiRef: MutableRefObject<ShardPickerState | null> = { current: null };
  render(
    <ShardPickerProvider>
      <ApiRef apiRef={apiRef} />
    </ShardPickerProvider>,
  );
  if (!apiRef.current) {
    throw new Error("picker did not mount");
  }
  return apiRef;
}

describe("ShardPickerProvider", () => {
  it("drops a selected id that coverage no longer lists", () => {
    const apiRef = mountPicker();
    act(() => {
      expect(apiRef.current?.toggle(2, "10")).toBe(true);
    });
    expect(apiRef.current?.isSelected(2)).toBe(true);
    act(() => {
      apiRef.current?.registerCoverage([row(1, "5")]);
    });
    expect(apiRef.current?.isSelected(2)).toBe(false);
    expect(apiRef.current?.selectedCount).toBe(0);
  });

  it("refreshes profit for ids still in coverage", () => {
    const apiRef = mountPicker();
    act(() => {
      apiRef.current?.toggle(2, "10");
    });
    act(() => {
      apiRef.current?.registerCoverage([row(2, "20")]);
    });
    expect(apiRef.current?.isSelected(2)).toBe(true);
    expect(apiRef.current?.expectedProfitSumAtomic).toBe(20n);
  });

  it("sums profits above Number.MAX_SAFE_INTEGER without rounding", () => {
    const apiRef = mountPicker();
    act(() => {
      apiRef.current?.toggle(1, "9007199254740993");
      apiRef.current?.toggle(2, "9007199254740993");
    });
    expect(apiRef.current?.expectedProfitSumAtomic).toBe(18014398509481986n);
  });

  it("rejects an add from current state once the holdings cap is full", () => {
    const apiRef = mountPicker();
    act(() => {
      for (let i = 0; i < MAX_HOLDINGS_SHARDS; i += 1) {
        apiRef.current?.toggle(i, "1");
      }
    });
    expect(apiRef.current?.selectedCount).toBe(MAX_HOLDINGS_SHARDS);
    expect(apiRef.current?.atCap).toBe(true);
    let accepted = true;
    act(() => {
      accepted = apiRef.current?.toggle(MAX_HOLDINGS_SHARDS, "1") ?? true;
    });
    expect(accepted).toBe(false);
    expect(apiRef.current?.selectedCount).toBe(MAX_HOLDINGS_SHARDS);
  });
});
