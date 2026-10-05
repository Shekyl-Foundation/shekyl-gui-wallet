import { useRef, useState } from "react";
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
  /** Bumped on every edit, so a check that finishes late cannot repaint. */
  const claim = useRef(0);

  const edit = (set: (value: string) => void) => (event: { target: { value: string } }) => {
    claim.current += 1;
    set(event.target.value);
    setNotice(null);
  };

  const run = async (work: () => Promise<ProofNotice>) => {
    const ticket = claim.current;
    setBusy(true);
    setNotice(null);
    try {
      const next = await work();
      if (ticket !== claim.current) return;
      setNotice(next);
    } catch (err) {
      if (ticket !== claim.current) return;
      setNotice({ kind: "fault", text: describeError(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card space-y-2">
      <p className="text-xs font-semibold text-purple-200">Payment</p>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="payment-txid">
          Transaction id
        </label>
        <input
          id="payment-txid"
          className="input"
          placeholder="64 hex characters"
          value={txid}
          onChange={edit(setTxid)}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="payment-address">
          Address
        </label>
        <input
          id="payment-address"
          className="input"
          placeholder="shekyl1..."
          value={address}
          onChange={edit(setAddress)}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="payment-message">
          Message <span className="text-purple-400">(optional)</span>
        </label>
        <input
          id="payment-message"
          className="input"
          value={message}
          onChange={edit(setMessage)}
        />
      </div>
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
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="payment-proof">
          Payment proof
        </label>
        <textarea
          id="payment-proof"
          className="input min-h-28 font-mono text-xs"
          value={proof}
          onChange={edit(setProof)}
        />
      </div>
      <ProofNoticeLine notice={notice} />
    </div>
  );
}
