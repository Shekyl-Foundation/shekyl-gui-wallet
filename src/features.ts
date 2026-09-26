import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Compiled feature switches, read from Rust at boot.
 *
 * There is ONE switch per feature — the cargo feature on `src-tauri` — and
 * the UI gates its surfaces on what Rust reports, never on a flag of its own
 * that could disagree with what was actually compiled. Until Rust answers
 * (or if it cannot) every gated surface stays OFF: a gate that fails open
 * would render a page whose commands are not registered.
 */
export interface FeatureFlags {
  multisig: boolean;
}

const FAIL_CLOSED: FeatureFlags = { multisig: false };

let cached: Promise<FeatureFlags> | null = null;

function isFeatureFlags(v: unknown): v is FeatureFlags {
  return typeof v === "object" && v !== null && typeof (v as FeatureFlags).multisig === "boolean";
}

export function fetchFeatureFlags(): Promise<FeatureFlags> {
  if (!cached) {
    // Start from a settled promise so a synchronous throw, a non-promise
    // return, or a malformed answer all end in FAIL_CLOSED rather than in a
    // rendered surface whose commands may not be registered.
    cached = Promise.resolve()
      .then(() => invoke<FeatureFlags>("get_feature_flags"))
      .then((f) => (isFeatureFlags(f) ? f : FAIL_CLOSED))
      .catch(() => FAIL_CLOSED);
  }
  return cached;
}

export function useFeatureFlags(): FeatureFlags {
  const [flags, setFlags] = useState<FeatureFlags>(FAIL_CLOSED);
  useEffect(() => {
    let live = true;
    void fetchFeatureFlags().then((f) => {
      if (live) setFlags(f);
    });
    return () => {
      live = false;
    };
  }, []);
  return flags;
}

/** Test seam: forget the cached answer so each test controls the mock. */
export function resetFeatureFlagsForTests(): void {
  cached = null;
}
