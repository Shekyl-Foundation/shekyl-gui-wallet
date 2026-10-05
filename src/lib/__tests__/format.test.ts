import { describe, it, expect } from "vitest";
import {
  atomicAmount,
  formatSkl,
  parseSkl,
  SKL_AMOUNT_PATTERN,
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

  it("does not round a string above 2^53 the way JSON number would", () => {
    expect(formatSkl("9007199254740993", 9)).toBe("9007199.254740993");
    const asJsonNumber = JSON.parse("9007199254740993") as number;
    expect(asJsonNumber).toBe(Number.MAX_SAFE_INTEGER + 1);
    expect(formatSkl(asJsonNumber, 9)).toBe("9007199.254740992");
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

describe("atomicAmount", () => {
  it("keeps 2^53+1 exact from a decimal string", () => {
    expect(atomicAmount("9007199254740993")).toBe(9007199254740993n);
  });

  it("rejects a non-integer string", () => {
    expect(() => atomicAmount("1.5")).toThrow(/decimal integer/);
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

describe("parseSkl", () => {
  it("is exact at and beyond 2^53 atomic units", () => {
    // 2^53 + 1 atomic: the first value a JS number cannot hold.
    expect(parseSkl("9007199.254740993")).toBe(9007199254740993n);
    expect(parseSkl("4294967296")).toBe(4294967296n * 1_000_000_000n);
  });

  it("pads short fractions and accepts a bare integer", () => {
    expect(parseSkl("1.5")).toBe(1_500_000_000n);
    expect(parseSkl("12")).toBe(12_000_000_000n);
    expect(parseSkl("12.")).toBe(12_000_000_000n);
    expect(parseSkl("0.000000001")).toBe(1n);
  });

  it("refuses what it cannot represent rather than truncating", () => {
    for (const bad of ["0.0000000001", "", "abc", "-1", ".5", "1e9", "1,5"]) {
      expect(() => parseSkl(bad), bad).toThrow(RangeError);
    }
  });

  it("round-trips through formatSkl at full precision", () => {
    for (const s of ["0.000000001", "1.5", "9007199.254740993"]) {
      expect(formatSkl(parseSkl(s), 9)).toBe(
        s.includes(".") ? s.padEnd(s.indexOf(".") + 1 + 9, "0") : `${s}.000000000`,
      );
    }
  });
});

describe("SKL_AMOUNT_PATTERN", () => {
  it("is the same grammar parseSkl accepts", () => {
    const pattern = new RegExp(`^(?:${SKL_AMOUNT_PATTERN})$`);
    for (const s of ["12", "12.", "1.5", "0.000000001", "9007199.254740993"]) {
      expect(pattern.test(s), s).toBe(true);
      expect(() => parseSkl(s), s).not.toThrow();
    }
    for (const s of ["0.0000000001", ".5", "-1", "1e9", "1,5", ""]) {
      expect(pattern.test(s), s).toBe(false);
      expect(() => parseSkl(s), s).toThrow(RangeError);
    }
  });
});
