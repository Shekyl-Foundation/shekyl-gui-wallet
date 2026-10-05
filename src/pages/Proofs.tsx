import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseSkl } from "../lib/format";

function messageOf(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  return e instanceof Error ? e.message : String(e);
}

/** Prove and check a payment or a reserve. The proof string is what you share. */
export default function Proofs() {
  const [txid, setTxid] = useState("");
  const [address, setAddress] = useState("");
  const [message, setMessage] = useState("");
  const [proof, setProof] = useState("");
  const [amount, setAmount] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      await work();
    } catch (e) {
      setError(messageOf(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <h1 className="text-xl font-bold text-white">Proofs</h1>
      <p className="text-xs text-purple-300">
        A payment proof shows that a transaction paid an address. A reserve proof
        shows that this wallet holds funds. Anyone who receives the proof string
        can check it. An outbound payment proof also reveals the transaction key.
        A reserve proof reveals amounts and lets a holder see when those outputs
        are spent.
      </p>
      <div className="card space-y-2">
        <input className="input" placeholder="Transaction id" value={txid} onChange={(e) => setTxid(e.target.value)} />
        <input className="input" placeholder="Address" value={address} onChange={(e) => setAddress(e.target.value)} />
        <input className="input" placeholder="Message (optional)" value={message} onChange={(e) => setMessage(e.target.value)} />
        <div className="flex gap-2">
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const result = await invoke<{ proof: string; direction: string }>("get_tx_proof", {
                  txid,
                  address,
                  message,
                });
                setProof(result.proof);
                setNote(`Payment proof (${result.direction}). Share the string below.`);
              })
            }
          >
            Prove payment
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const result = await invoke<{ valid: boolean; direction?: string }>("check_tx_proof", {
                  txid,
                  address,
                  proof,
                  message,
                });
                setNote(result.valid ? `Payment proof is good (${result.direction ?? ""}).` : "Payment proof does not verify.");
              })
            }
          >
            Check payment
          </button>
        </div>
      </div>
      <div className="card space-y-2">
        <input
          className="input"
          placeholder="Amount in SKL, or empty for the whole balance"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
        />
        <div className="flex gap-2">
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const atomic = amount.trim().length === 0 ? undefined : parseSkl(amount).toString();
                const result = await invoke<{ proof: string }>("get_reserve_proof", {
                  amount: atomic,
                  message,
                });
                setProof(result.proof);
                setNote("Reserve proof created. Share it only with the person who must see it.");
              })
            }
          >
            Prove reserve
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const result = await invoke<{ valid: boolean }>("check_reserve_proof", {
                  address,
                  proof,
                  message,
                });
                setNote(result.valid ? "Reserve proof is good." : "Reserve proof does not verify.");
              })
            }
          >
            Check reserve
          </button>
        </div>
      </div>
      <textarea className="input min-h-28 font-mono text-xs" value={proof} onChange={(e) => setProof(e.target.value)} />
      {error && <p className="text-xs text-red-300">{error}</p>}
      {note && <p className="text-xs text-emerald-200">{note}</p>}
    </div>
  );
}
