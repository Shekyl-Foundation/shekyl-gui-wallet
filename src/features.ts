import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Compiled feature switches, as `get_feature_flags` reports them.
 * One field per cargo feature the UI can show.
 */
export interface FeatureFlags {
  multisig: boolean;
}

/** A nav row or glossary entry that exists only under a compiled feature. */
export interface GatedSurface {
  feature?: keyof FeatureFlags;
}

/**
 * Loading, a parsed answer, or a fail-closed fault.
 * A fault is cached for the page lifetime: a later answer must not open a
 * surface the first answer could not prove was compiled in.
 */
export type FeatureGateState =
  | { kind: "loading" }
  | { kind: "ready"; flags: FeatureFlags }
  | { kind: "fault" };

const FAULT: FeatureGateState = { kind: "fault" };

let cached: Promise<FeatureGateState> | null = null;

function readFeatureFlags(value: unknown): FeatureFlags | null {
  if (typeof value !== "object" || value === null) return null;
  const multisig = Reflect.get(value, "multisig");
  if (typeof multisig !== "boolean") return null;
  return { multisig };
}

/**
 * True when `feature` is absent (the surface is always on) or Rust has
 * reported that feature. Loading and fault are both off.
 */
export function isSurfaceEnabled(
  state: FeatureGateState,
  feature: keyof FeatureFlags | undefined,
): boolean {
  if (feature === undefined) return true;
  return state.kind === "ready" && state.flags[feature];
}

export function fetchFeatureGate(): Promise<FeatureGateState> {
  if (!cached) {
    cached = Promise.resolve()
      .then(() => invoke("get_feature_flags"))
      .then((value) => {
        const flags = readFeatureFlags(value);
        return flags === null ? FAULT : { kind: "ready" as const, flags };
      })
      .catch(() => FAULT);
  }
  return cached;
}

export function useFeatureGate(): FeatureGateState {
  const [state, setState] = useState<FeatureGateState>({ kind: "loading" });
  useEffect(() => {
    let live = true;
    void fetchFeatureGate().then((next) => {
      if (live) setState(next);
    });
    return () => {
      live = false;
    };
  }, []);
  return state;
}

/** Test seam: forget the cached answer so each test controls the mock. */
export function resetFeatureFlagsForTests(): void {
  cached = null;
}
