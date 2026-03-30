import { useDaemon } from "../context/useDaemon";
import { AlertTriangle, FlaskConical } from "lucide-react";

export default function TestnetBanner() {
  const { health } = useDaemon();

  if (!health || health.network === "mainnet") return null;

  const isTestnet = health.network === "testnet";

  return (
    <div
      className={`flex items-center justify-center gap-2 px-4 py-1.5 text-xs font-semibold ${
        isTestnet
          ? "bg-amber-500/20 text-amber-300"
          : "bg-blue-500/20 text-blue-300"
      }`}
    >
      {isTestnet ? (
        <>
          <AlertTriangle className="h-3.5 w-3.5" />
          <span>TestNet Active — Mainnet July 2026</span>
          <a
            href="https://faucet.shekyl.org"
            target="_blank"
            rel="noopener noreferrer"
            className="ml-2 underline decoration-dotted hover:decoration-solid"
          >
            Get test SKL
          </a>
        </>
      ) : (
        <>
          <FlaskConical className="h-3.5 w-3.5" />
          <span>StageNet — For integration testing</span>
        </>
      )}
    </div>
  );
}
