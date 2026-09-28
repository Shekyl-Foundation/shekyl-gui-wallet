import { useEffect, useRef, useState } from "react";

export type CopyState = "idle" | "copied" | "failed";

/** How long "Copied" / "Copy failed" stays before the button resets. */
const FEEDBACK_MS = 2000;

/**
 * Copy text to the clipboard and say what happened. `navigator.clipboard`
 * can reject (permissions, insecure context, OS clipboard failure); a
 * silent rejection leaves the person unsure whether anything was copied,
 * so the failure is a state the button shows, not a swallowed promise.
 */
export function useCopyFeedback(): { state: CopyState; copy: (text: string) => void } {
  const [state, setState] = useState<CopyState>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  function show(next: CopyState) {
    setState(next);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), FEEDBACK_MS);
  }

  function copy(text: string) {
    const clipboard = typeof navigator !== "undefined" ? navigator.clipboard : undefined;
    if (!clipboard?.writeText) {
      show("failed");
      return;
    }
    clipboard.writeText(text).then(
      () => show("copied"),
      () => show("failed"),
    );
  }

  return { state, copy };
}
