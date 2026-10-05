import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeError } from "../lib/errors";

type Notice = { kind: "ready" | "fault"; text: string };

/** Sign a message with this wallet, or check someone else's signature. */
export default function Sign() {
  const [message, setMessage] = useState("");
  const [address, setAddress] = useState("");
  const [signature, setSignature] = useState("");
  const [notice, setNotice] = useState<Notice | null>(null);
  const [busy, setBusy] = useState(false);

  const sign = async () => {
    setBusy(true);
    setNotice({ kind: "ready", text: "Signing takes a few seconds." });
    try {
      const result = await invoke<{ signature: string }>("sign_message", { message });
      setSignature(result.signature);
      setNotice({
        kind: "ready",
        text: "Signed. Share the message, this signature, and your address.",
      });
    } catch (err) {
      setNotice({ kind: "fault", text: describeError(err) });
    } finally {
      setBusy(false);
    }
  };

  const verify = async () => {
    setBusy(true);
    setNotice(null);
    try {
      await invoke("verify_message", { address, message, signature });
      setNotice({
        kind: "ready",
        text: "This signature matches the address and the message.",
      });
    } catch (err) {
      setNotice({ kind: "fault", text: describeError(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-2xl space-y-4">
      <h1 className="text-xl font-bold text-white">Sign</h1>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="sign-message">
          Message
        </label>
        <textarea
          id="sign-message"
          className="input min-h-24"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="sign-address">
          Address <span className="text-purple-400">(for checking)</span>
        </label>
        <input
          id="sign-address"
          className="input"
          placeholder="shekyl1..."
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-xs font-medium text-purple-200" htmlFor="sign-signature">
          Signature
        </label>
        <textarea
          id="sign-signature"
          className="input min-h-24 font-mono text-xs"
          value={signature}
          onChange={(e) => setSignature(e.target.value)}
        />
      </div>
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
      {notice && (
        <p className={notice.kind === "ready" ? "text-xs text-emerald-200" : "text-xs text-red-300"}>
          {notice.text}
        </p>
      )}
    </div>
  );
}
