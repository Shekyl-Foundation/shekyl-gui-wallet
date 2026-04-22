import { WifiOff } from "lucide-react";
import { useDaemon } from "../context/useDaemon";

// Sticky banner shown after the wallet has connected at least once and then
// lost the daemon. Until the first connect, users see the full
// `DaemonConnectingScreen` instead; this banner is only about transient
// disconnections post-ready.
export default function DaemonOfflineBanner() {
  const { connected, hasEverConnected, status } = useDaemon();

  if (!hasEverConnected || connected) return null;

  return (
    <div className="flex items-center justify-center gap-2 bg-red-500/20 px-4 py-1.5 text-xs font-semibold text-red-200">
      <WifiOff className="h-3.5 w-3.5" />
      <span>
        Daemon unreachable — reconnecting
        {status?.daemon_address && (
          <span className="ml-1 font-mono text-red-100/80">
            ({status.daemon_address})
          </span>
        )}
      </span>
    </div>
  );
}
