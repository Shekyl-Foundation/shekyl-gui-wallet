const ATOMIC_PER_SKL = 1_000_000_000n;
const MILLION_SKL_ATOMIC = 1_000_000n * ATOMIC_PER_SKL;
const THOUSAND_SKL_ATOMIC = 1_000n * ATOMIC_PER_SKL;
const ATOMIC_DIVISOR = 1e9;
const FIXED_POINT_SCALE = 1_000_000;
const MAX_SUPPLY = 4_294_967_296;
const ATOMIC_INTEGER = /^-?\d+$/;

/** Atomic units from the Tauri wire (`string` / `bigint`) or remaining JSON-number DTOs. */
export type AtomicAmount = bigint | string | number;

export function atomicAmount(atomic: AtomicAmount): bigint {
  if (typeof atomic === "bigint") {
    return atomic;
  }
  if (typeof atomic === "string") {
    const t = atomic.trim();
    if (!ATOMIC_INTEGER.test(t)) {
      throw new RangeError(`atomic amount is not a decimal integer: ${atomic}`);
    }
    return BigInt(t);
  }
  if (!Number.isFinite(atomic)) {
    throw new RangeError("atomic amount is not finite");
  }
  return BigInt(Math.trunc(atomic));
}

function formatHundredths(hundredths: bigint): string {
  const sign = hundredths < 0n ? "-" : "";
  const abs = hundredths < 0n ? -hundredths : hundredths;
  const whole = abs / 100n;
  const frac = (abs % 100n).toString().padStart(2, "0");
  return `${sign}${whole}.${frac}`;
}

export function formatSkl(atomic: AtomicAmount, precision: 6 | 9 = 6): string {
  const n = atomicAmount(atomic);
  const sign = n < 0n ? "-" : "";
  const abs = n < 0n ? -n : n;
  const whole = abs / ATOMIC_PER_SKL;
  const frac = (abs % ATOMIC_PER_SKL)
    .toString()
    .padStart(9, "0")
    .slice(0, precision);
  return `${sign}${whole}.${frac}`;
}

export function formatSklCompact(atomic: AtomicAmount): string {
  const n = atomicAmount(atomic);
  const abs = n < 0n ? -n : n;
  if (abs >= MILLION_SKL_ATOMIC) {
    return `${formatHundredths((n * 100n) / MILLION_SKL_ATOMIC)}M`;
  }
  if (abs >= THOUSAND_SKL_ATOMIC) {
    return `${formatHundredths((n * 100n) / THOUSAND_SKL_ATOMIC)}K`;
  }
  return formatSkl(n);
}

export function formatPercent(fixedPoint: number, scale = FIXED_POINT_SCALE): string {
  const pct = (fixedPoint / scale) * 100;
  return `${pct.toFixed(2)}%`;
}

export function formatMultiplier(fixedPoint: number, scale = FIXED_POINT_SCALE): string {
  const value = fixedPoint / scale;
  return `${value.toFixed(2)}x`;
}

export function formatHashRate(difficulty: number, blockTimeSeconds = 120): string {
  const hashRate = difficulty / blockTimeSeconds;
  if (hashRate >= 1e12) return `${(hashRate / 1e12).toFixed(2)} TH/s`;
  if (hashRate >= 1e9) return `${(hashRate / 1e9).toFixed(2)} GH/s`;
  if (hashRate >= 1e6) return `${(hashRate / 1e6).toFixed(2)} MH/s`;
  if (hashRate >= 1e3) return `${(hashRate / 1e3).toFixed(2)} KH/s`;
  return `${hashRate.toFixed(0)} H/s`;
}

export function formatDuration(hours: number): string {
  if (hours >= 24 * 30) return `${Math.round(hours / (24 * 30))} months`;
  if (hours >= 24) return `${Math.round(hours / 24)} days`;
  return `${Math.round(hours)} hours`;
}

export function emissionProgress(generatedCoins: string): number {
  const generated = parseInt(generatedCoins, 10) || 0;
  const supply = generated / ATOMIC_DIVISOR;
  return Math.min((supply / MAX_SUPPLY) * 100, 100);
}
