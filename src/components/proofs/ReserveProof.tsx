import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../../lib/errors";
import { formatSkl, parseSkl, SKL_DECIMALS } from "../../lib/format";
import { ProofNoticeLine, type ProofNotice } from "./notice";

interface ReserveCheck {
  valid: boolean;
  total?: string;
  spent?: string;
  output_count?: number;
}

function checkedReserve(result: ReserveCheck): ProofNotice {
  if (!result.valid) {
    return { kind: "fault", text: "This reserve proof does not check out." };
  }
  if (result.total === undefined || result.spent === undefined) {
    return { kind: "ready", text: "This reserve proof checks out." };
  }
  const outputs =
    result.output_count === undefined
      ? ""
      : result.output_count === 1
        ? " across 1 output"
        : ` across ${result.output_count} outputs`;
  return {
    kind: "ready",
    text:
      `This reserve proof checks out. It covers ${formatSkl(result.total, SKL_DECIMALS)} SKL, ` +
      `of which ${formatSkl(result.spent, SKL_DECIMALS)} SKL is already spent${outputs}.`,
  };
}

/** Prove or check one reserve. Its fields are not the payment panel's. */
export default function ReserveProof() {
  const [amount, setAmount] = useState("");
  const [address, setAddress] = useState("");
  const [message, setMessage] = useState("");
  const [proof, setProof] = useState("");
  const [notice, setNotice] = useState<ProofNotice | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (work: () => Promise<ProofNotice>) => {
    setBusy(true);
    setNotice(null);
    try {
      setNotice(await work());
    } catch (err) {
      setNotice({ kind: "fault", text: describeError(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-2">
      <p className="text-xs font-semibold text-purple-200">Reserve</p>
      <p className="text-xs text-purple-300">
        A reserve proof shows that an address holds funds. It reveals amounts,
        and lets the holder see when those outputs are spent. Share it only
        with the person who must see it.
      </p>
      <input
        className="input"
        placeholder="Amount in SKL, or empty for the whole balance"
        value={amount}
        onChange={(e) => setAmount(e.target.value)}
      />
      <input
        className="input"
        placeholder="Message (optional)"
        value={message}
        onChange={(e) => setMessage(e.target.value)}
      />
      <input
        className="input"
        placeholder="Address, for checking"
        value={address}
        onChange={(e) => setAddress(e.target.value)}
      />
      <div className="flex gap-2">
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy}
          onClick={() =>
            void run(async () => {
              const atomic = amount.trim().length === 0 ? undefined : parseSkl(amount).toString();
              const result = await invoke<{ proof: string; total: string; output_count: number }>(
                "get_reserve_proof",
                { amount: atomic, message },
              );
              setProof(result.proof);
              const outputs = result.output_count === 1 ? "1 output" : `${result.output_count} outputs`;
              return {
                kind: "ready",
                text: `Reserve proof created for ${formatSkl(result.total, SKL_DECIMALS)} SKL across ${outputs}.`,
              };
            })
          }
        >
          Prove reserve
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={busy || address.length === 0 || proof.length === 0}
          onClick={() =>
            void run(async () => {
              const result = await invoke<ReserveCheck>("check_reserve_proof", {
                address,
                proof,
                message,
              });
              return checkedReserve(result);
            })
          }
        >
          Check reserve
        </button>
      </div>
      <textarea
        className="input min-h-28 font-mono text-xs"
        placeholder="Reserve proof"
        value={proof}
        onChange={(e) => setProof(e.target.value)}
      />
      <ProofNoticeLine notice={notice} />
    </div>
  );
}
