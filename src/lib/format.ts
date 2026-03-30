const ATOMIC_DIVISOR = 1e9;
const FIXED_POINT_SCALE = 1_000_000;
const MAX_SUPPLY = 4_294_967_296;

export function formatSkl(atomic: number, precision: 6 | 9 = 6): string {
  const value = atomic / ATOMIC_DIVISOR;
  const factor = Math.pow(10, precision);
  const truncated = Math.trunc(value * factor) / factor;
  return truncated.toFixed(precision);
}

export function formatSklCompact(atomic: number): string {
  const value = atomic / ATOMIC_DIVISOR;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(2)}K`;
  return formatSkl(atomic);
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
