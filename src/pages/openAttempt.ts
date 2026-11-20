import { createContext, useContext } from "react";
import type { OpenRefusal } from "../types/wallet";

/**
 * One open that refused, bound to the wallet file it was tried against.
 * The unlock-phase route shell holds this, so it outlives the Unlock page:
 * the create screen is a child of that shell, and coming back still shows
 * this file's sentence.
 */
export interface OpenAttempt {
  file: string;
  refusal: OpenRefusal;
}

export interface OpenAttemptStore {
  attempt: OpenAttempt | null;
  recordAttempt: (attempt: OpenAttempt) => void;
  clearAttempt: () => void;
}

export const OpenAttemptContext = createContext<OpenAttemptStore | null>(null);

export function useOpenAttempt(): OpenAttemptStore {
  const store = useContext(OpenAttemptContext);
  if (store === null) {
    throw new Error("useOpenAttempt requires OpenAttemptProvider");
  }
  return store;
}
