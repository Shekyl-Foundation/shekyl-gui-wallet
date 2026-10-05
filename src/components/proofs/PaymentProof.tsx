import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../../lib/errors";
import { formatSkl, SKL_DECIMALS } from "../../lib/format";
import { ProofNoticeLine, type ProofNotice } from "./notice";

interface TxProofOutput {
  output_index: number;
  amount: string;
}

interface TxCheck {
  valid: boolean;
  direction?: string;
  received?: string;
  outputs?: TxProofOutput[];
  in_pool?: boolean;
  confirmations?: number;
}

function paymentDirection(direction: string | undefined): string {
  if (direction === "OUTBOUND") return "a payment this wallet sent";
  if (direction === "INBOUND") return "a payment this wallet received";
  return "this payment";
}

function checkedPayment(result: TxCheck): ProofNotice {
  if (!result.valid) {
    return { kind: "fault", text: "This payment proof does not check out." };
  }
  const parts: string[] = [];
  if (result.received !== undefined) {
    parts.push(`${formatSkl(result.received, SKL_DECIMALS)} SKL`);
  }
  if (result.outputs !== undefined) {
    const count = result.outputs.length;
    parts.push(count === 1 ? "1 output" : `${count} outputs`);
  }
  if (result.confirmations !== undefined) {
    parts.push(
      result.confirmations === 1 ? "1 confirmation" : `${result.confirmations} confirmations`,
    );
  }
  const detail = parts.length > 0 ? `: ${parts.join(", ")}` : "";
  const pooled = result.in_pool ? " It is not in a block yet." : "";
  return {
    kind: "ready",
    text: `This proof checks out for ${paymentDirection(result.direction)}${detail}.${pooled}`,
  };
}

/** Prove or check one payment. Its fields are not the reserve panel's. */
export default function PaymentProof() {
  const [txid, setTxid] = useState("");
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
      <p className="text-xs font-semibold text-purple-200">Payment</p>
      <input className="input" placeholder="Transaction id" value={txid} onChange={(e) => setTxid(e.target.value)} />
      <input className="input" placeholder="Address" value={address} onChange={(e) => setAddress(e.target.value)} />
      <input
        className="input"
        placeholder="Message (optional)"
        value={message}
        onChange={(e) => setMessage(e.target.value)}
      />
      <div className="flex gap-2">
        <button
          type="button"
          className="btn btn-primary"
          disabled={busy || txid.length === 0 || address.length === 0}
          onClick={() =>
            void run(async () => {
              const result = await invoke<{ proof: string; direction: string }>("get_tx_proof", {
                txid,
                address,
                message,
              });
              setProof(result.proof);
              return {
                kind: "ready",
                text: `Payment proof created for ${paymentDirection(result.direction)}. Share the string below only with the person who must see it. An outbound proof also reveals the transaction key.`,
              };
            })
          }
        >
          Prove payment
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={busy || txid.length === 0 || address.length === 0 || proof.length === 0}
          onClick={() =>
            void run(async () => {
              const result = await invoke<TxCheck>("check_tx_proof", { txid, address, proof, message });
              return checkedPayment(result);
            })
          }
        >
          Check payment
        </button>
      </div>
      <textarea
        className="input min-h-28 font-mono text-xs"
        placeholder="Payment proof"
        value={proof}
        onChange={(e) => setProof(e.target.value)}
      />
      <ProofNoticeLine notice={notice} />
    </div>
  );
}
