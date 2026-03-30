import { describe, it, expect } from "vitest";
import {
  formatSkl,
  formatSklCompact,
  formatPercent,
  formatMultiplier,
  formatHashRate,
  formatDuration,
  emissionProgress,
} from "../format";

describe("formatSkl", () => {
  it("converts atomic units using 1e9 divisor with 6 decimal truncation", () => {
    expect(formatSkl(5_000_000_000)).toBe("5.000000");
    expect(formatSkl(1_234_567_890)).toBe("1.234567");
    expect(formatSkl(999)).toBe("0.000000");
  });

  it("supports 9-decimal precision", () => {
    expect(formatSkl(1_234_567_891, 9)).toBe("1.234567891");
  });

  it("truncates rather than rounds", () => {
    expect(formatSkl(1_999_999_999)).toBe("1.999999");
  });

  it("handles zero", () => {
    expect(formatSkl(0)).toBe("0.000000");
  });
});

describe("formatSklCompact", () => {
  it("formats millions with M suffix", () => {
    expect(formatSklCompact(2_500_000_000_000_000)).toBe("2.50M");
  });

  it("formats thousands with K suffix", () => {
    expect(formatSklCompact(5_000_000_000_000)).toBe("5.00K");
  });

  it("formats small values as regular SKL", () => {
    expect(formatSklCompact(500_000_000)).toBe("0.500000");
  });
});

describe("formatPercent", () => {
  it("converts fixed-point (SCALE=1M) to percentage", () => {
    expect(formatPercent(50_000)).toBe("5.00%");
    expect(formatPercent(1_000_000)).toBe("100.00%");
  });
});

describe("formatMultiplier", () => {
  it("converts fixed-point to Nx format", () => {
    expect(formatMultiplier(1_120_000)).toBe("1.12x");
    expect(formatMultiplier(2_000_000)).toBe("2.00x");
  });
});

describe("formatHashRate", () => {
  it("formats H/s for small values", () => {
    expect(formatHashRate(1200)).toBe("10 H/s");
  });

  it("formats MH/s for medium values", () => {
    expect(formatHashRate(120_000_000)).toBe("1.00 MH/s");
  });

  it("formats GH/s for large values", () => {
    expect(formatHashRate(120_000_000_000)).toBe("1.00 GH/s");
  });
});

describe("formatDuration", () => {
  it("formats hours", () => {
    expect(formatDuration(12)).toBe("12 hours");
  });

  it("formats days", () => {
    expect(formatDuration(72)).toBe("3 days");
  });

  it("formats months", () => {
    expect(formatDuration(24 * 90)).toBe("3 months");
  });
});

describe("emissionProgress", () => {
  it("calculates percentage of max supply", () => {
    const generated = String(1_000_000_000 * 1_000_000_000);
    const progress = emissionProgress(generated);
    expect(progress).toBeGreaterThan(0);
    expect(progress).toBeLessThanOrEqual(100);
  });

  it("returns 0 for empty input", () => {
    expect(emissionProgress("")).toBe(0);
  });
});
