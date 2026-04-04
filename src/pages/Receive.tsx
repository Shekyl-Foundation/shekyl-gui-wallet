import { useEffect, useState, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Check, ChevronDown, ChevronUp, ShieldCheck } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";

const BECH32M_PREFIX = "shekyl1";
const CLASSICAL_SEGMENT_LEN = 95;

function splitAddress(full: string): {
  classical: string;
  pqSegment: string | null;
} {
  if (!full.startsWith(BECH32M_PREFIX)) {
    return { classical: full, pqSegment: null };
  }
  if (full.length <= CLASSICAL_SEGMENT_LEN) {
    return { classical: full, pqSegment: null };
  }
  return {
    classical: full.slice(0, CLASSICAL_SEGMENT_LEN),
    pqSegment: full.slice(CLASSICAL_SEGMENT_LEN),
  };
}

export default function Receive() {
  const [address, setAddress] = useState<string>("");
  const [copied, setCopied] = useState(false);
  const [showFull, setShowFull] = useState(false);

  useEffect(() => {
    invoke<string>("get_address", { account: 0, index: 0 })
      .then(setAddress)
      .catch(() => {});
  }, []);

  const { classical, pqSegment } = useMemo(
    () => splitAddress(address),
    [address],
  );
  const hasPqSegment = pqSegment !== null;

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
        {/* QR code encodes the full FCMP++ address (~1870 chars, QR version 20+ M) */}
        <div className="mx-auto flex items-center justify-center rounded-xl bg-white p-3">
          {address ? (
            <QRCodeSVG
              value={address}
              size={192}
              level="M"
              marginSize={0}
              bgColor="#ffffff"
              fgColor="#1a1025"
            />
          ) : (
            <div className="flex h-48 w-48 items-center justify-center text-xs text-purple-400">
              No wallet open
            </div>
          )}
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-center gap-2">
            <p className="text-xs text-purple-300">Your address</p>
            {hasPqSegment && (
              <span className="inline-flex items-center gap-1 rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[9px] font-semibold text-emerald-300">
                <ShieldCheck className="h-2.5 w-2.5" />
                FCMP++
              </span>
            )}
          </div>

          <div className="rounded-lg bg-purple-800 p-3">
            <div className="flex items-start gap-2">
              <code className="flex-1 break-all text-left text-xs text-gold-400">
                {address ? (
                  showFull || !hasPqSegment ? (
                    address
                  ) : (
                    <>
                      {classical}
                      <span className="text-purple-400">...</span>
                    </>
                  )
                ) : (
                  "No wallet open"
                )}
              </code>
              <button
                onClick={copyAddress}
                disabled={!address}
                className="btn-ghost shrink-0 rounded-md p-1.5"
                title="Copy full address"
              >
                {copied ? (
                  <Check className="h-4 w-4 text-emerald-400" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </button>
            </div>

            {hasPqSegment && (
              <button
                onClick={() => setShowFull(!showFull)}
                className="mt-2 inline-flex items-center gap-1 text-[10px] text-purple-300 hover:text-purple-100 transition-colors"
              >
                {showFull ? (
                  <>
                    <ChevronUp className="h-3 w-3" />
                    Show classical segment only
                  </>
                ) : (
                  <>
                    <ChevronDown className="h-3 w-3" />
                    Show full address (classical + PQ)
                  </>
                )}
              </button>
            )}
          </div>

          {hasPqSegment && (
            <p className="text-[10px] text-purple-400">
              Copy always includes the full FCMP++ address with post-quantum
              segment.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
