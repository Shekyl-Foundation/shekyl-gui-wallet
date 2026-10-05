import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PrimaryAddress } from "../../types/wallet";

export const ADDRESS_UNREAD =
  "The address could not be read. The wallet was created — open it, then copy the address from Receive.";

type AddressRead =
  | { kind: "loading" }
  | { kind: "ready"; address: string }
  | { kind: "fault" };

/**
 * The new wallet's address, read from its one source (`get_primary_address`)
 * once the wallet is open. A failed read is its own state with a retry,
 * never a blank address (rule 82). Owns the fetch; the Create page composes
 * it (rule 27).
 */
export default function CreatedWalletAddress() {
  const [read, setRead] = useState<AddressRead>({ kind: "loading" });
  /** Monotonic generation so a late response cannot overwrite a retry. */
  const gen = useRef(0);

  const readAddress = useCallback(() => {
    const mine = ++gen.current;
    setRead({ kind: "loading" });
    invoke<PrimaryAddress>("get_primary_address")
      .then((response) => {
        if (mine === gen.current) setRead({ kind: "ready", address: response.address });
      })
      .catch(() => {
        if (mine === gen.current) setRead({ kind: "fault" });
      });
  }, []);

  useEffect(() => {
    readAddress();
    return () => {
      gen.current += 1;
    };
  }, [readAddress]);

  return (
    <div className="space-y-2">
      <p className="text-xs text-purple-300">Address</p>
      {read.kind === "ready" ? (
        <p className="break-all rounded-lg bg-purple-800/80 px-3 py-2 font-mono text-[10px] text-gold-400">
          {read.address}
        </p>
      ) : read.kind === "fault" ? (
        <div className="space-y-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2">
          <p className="text-xs text-red-200">{ADDRESS_UNREAD}</p>
          <button
            type="button"
            onClick={readAddress}
            className="text-xs font-medium text-purple-200 underline underline-offset-2 hover:text-white"
          >
            Try again
          </button>
        </div>
      ) : (
        <p className="text-xs text-purple-400">Reading your address…</p>
      )}
    </div>
  );
}
