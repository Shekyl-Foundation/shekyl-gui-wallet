import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Check } from "lucide-react";

export default function Receive() {
  const [address, setAddress] = useState<string>("");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    invoke<string>("get_address", { account: 0, index: 0 })
      .then(setAddress)
      .catch(() => {});
  }, []);

  function copyAddress() {
    navigator.clipboard.writeText(address).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }

  return (
    <div className="mx-auto max-w-lg space-y-6">
      <h1 className="text-xl font-bold text-white">Receive SKL</h1>

      <div className="card space-y-5 text-center">
        {/* QR code placeholder */}
        <div className="mx-auto flex h-48 w-48 items-center justify-center rounded-xl border-2 border-dashed border-purple-600 bg-purple-800/40">
          <span className="text-xs text-purple-400">QR Code</span>
        </div>

        <div className="space-y-2">
          <p className="text-xs text-purple-300">Your address</p>
          <div className="flex items-center gap-2 rounded-lg bg-purple-800 p-3">
            <code className="flex-1 break-all text-xs text-gold-400">
              {address || "No wallet open"}
            </code>
            <button
              onClick={copyAddress}
              disabled={!address}
              className="btn-ghost rounded-md p-1.5"
              title="Copy address"
            >
              {copied ? (
                <Check className="h-4 w-4 text-emerald-400" />
              ) : (
                <Copy className="h-4 w-4" />
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
