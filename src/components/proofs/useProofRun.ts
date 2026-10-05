import { useRef, useState } from "react";
import { describeError } from "../../lib/errors";
import type { ProofNotice } from "./notice";

/** What one prove or check produced. `proof` is absent when the call only checked. */
export type ProofWork = { notice: ProofNotice; proof?: string };

/**
 * One in-flight prove or check. An edit bumps the claim, and a result is
 * applied only if that claim is still current — including the proof string,
 * so a late prove cannot replace what the person just typed.
 */
export function useProofRun(setProof: (proof: string) => void) {
  const [notice, setNotice] = useState<ProofNotice | null>(null);
  const [busy, setBusy] = useState(false);
  const claim = useRef(0);

  const edit = (set: (value: string) => void) => (event: { target: { value: string } }) => {
    claim.current += 1;
    set(event.target.value);
    setNotice(null);
  };

  const run = async (work: () => Promise<ProofWork>) => {
    const ticket = claim.current;
    setBusy(true);
    setNotice(null);
    try {
      const outcome = await work();
      if (ticket !== claim.current) return;
      if (outcome.proof !== undefined) setProof(outcome.proof);
      setNotice(outcome.notice);
    } catch (err) {
      if (ticket !== claim.current) return;
      setNotice({ kind: "fault", text: describeError(err) });
    } finally {
      setBusy(false);
    }
  };

  return { notice, busy, edit, run };
}
