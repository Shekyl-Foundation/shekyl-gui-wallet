import { useDaemon } from "../context/useDaemon";
import NetworkBadge from "./NetworkBadge";
import PqcStatusBadge from "./PqcStatusBadge";

export default function Header() {
  const { health, loading } = useDaemon();

  const network = health?.network ?? "mainnet";
  const connected = !!health;
  const synced = health?.synchronized ?? false;

  return (
    <header className="flex items-center justify-between border-b border-purple-700/50 bg-purple-900/60 px-6 py-3">
      <div className="flex items-center gap-3">
        <NetworkBadge network={network} />
        <PqcStatusBadge />
      </div>

      <div className="flex items-center gap-4 text-xs text-purple-300">
        {loading ? (
          <span className="flex items-center gap-1.5">
            <span className="h-2 w-2 animate-pulse rounded-full bg-purple-400" />
            Connecting...
          </span>
        ) : connected ? (
          <>
            <span className="flex items-center gap-1.5">
              <span
                className={`h-2 w-2 rounded-full ${
                  synced ? "bg-emerald-400" : "bg-amber-400"
                }`}
              />
              {synced ? "Synced" : "Syncing..."}
            </span>
            <span>Block {health!.height.toLocaleString()}</span>
          </>
        ) : (
          <span className="flex items-center gap-1.5">
            <span className="h-2 w-2 rounded-full bg-red-400" />
            Daemon unavailable
          </span>
        )}
      </div>
    </header>
  );
}
