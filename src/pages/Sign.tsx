import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function messageOf(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    const message = (e as { message: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  return e instanceof Error ? e.message : String(e);
}

/** Sign a message with this wallet, or check someone else's signature. */
export default function Sign() {
  const [message, setMessage] = useState("");
  const [address, setAddress] = useState("");
  const [signature, setSignature] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const sign = async () => {
    setBusy(true);
    setError(null);
    setNote("Signing takes a few seconds.");
    try {
      const result = await invoke<{ signature: string }>("sign_message", { message });
      setSignature(result.signature);
      setNote("Signed. Share the message, this signature, and your address.");
    } catch (e) {
      setError(messageOf(e));
      setNote(null);
    } finally {
      setBusy(false);
    }
  };

  const verify = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await invoke<{ valid: boolean }>("verify_message", {
        address,
        message,
        signature,
      });
      setNote(result.valid ? "Signature is valid for this address and message." : "Signature is not valid.");
    } catch (e) {
      setError(messageOf(e));
      setNote(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-2xl space-y-4">
      <h1 className="text-xl font-bold text-white">Sign</h1>
      <textarea className="input min-h-24" placeholder="Message" value={message} onChange={(e) => setMessage(e.target.value)} />
      <input className="input" placeholder="Address, for checking" value={address} onChange={(e) => setAddress(e.target.value)} />
      <textarea className="input min-h-24 font-mono text-xs" placeholder="Signature" value={signature} onChange={(e) => setSignature(e.target.value)} />
      <div className="flex gap-2">
        <button type="button" className="btn btn-primary" disabled={busy || message.length === 0} onClick={() => void sign()}>
          Sign
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={busy || message.length === 0 || address.length === 0 || signature.length === 0}
          onClick={() => void verify()}
        >
          Verify
        </button>
      </div>
      {error && <p className="text-xs text-red-300">{error}</p>}
      {note && <p className="text-xs text-emerald-200">{note}</p>}
    </div>
  );
}
