import { Loader2, ShieldCheck, Settings, Lock } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useDaemon } from "../context/useDaemon";
import { useWallet } from "../context/useWallet";

// After this many seconds without a successful connect we reveal secondary
// copy that acknowledges the most common causes (daemon still booting,
// testnet offline, incompatible seeds). Chosen empirically: `shekyld` cold
// start + first peer handshake is typically a handful of seconds on a
// healthy network, so anything past ~20s is almost certainly a real
// connectivity problem rather than normal startup.
const SECONDARY_COPY_THRESHOLD_SECONDS = 20;

export default function DaemonConnectingScreen() {
  const { status, waitingSeconds } = useDaemon();
  const { lockWallet } = useWallet();
  const navigate = useNavigate();

  const daemonUrl = status?.daemon_address ?? "(unknown)";
  const network = status?.network ?? "";
  const showSecondary = waitingSeconds >= SECONDARY_COPY_THRESHOLD_SECONDS;

  return (
    <div className="flex h-full w-full items-center justify-center px-6 py-12">
      <div className="w-full max-w-md space-y-6 text-center">
        <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-gold-500/20">
          <ShieldCheck className="h-8 w-8 text-gold-400" />
        </div>

        <div className="space-y-2">
          <h1 className="text-lg font-bold text-white">Connecting to shekyld</h1>
          <p className="flex items-center justify-center gap-2 text-xs text-purple-300">
            <Loader2 className="h-3.5 w-3.5 animate-spin text-gold-400" />
            <span>
              Waiting for the daemon to answer
              {waitingSeconds > 0 && ` · ${waitingSeconds}s`}
            </span>
          </p>
        </div>

        <div className="card space-y-2 text-left text-xs">
          <div className="flex items-center justify-between gap-3">
            <span className="text-purple-400">Daemon</span>
            <span className="break-all font-mono text-purple-100">
              {daemonUrl}
            </span>
          </div>
          {network && (
            <div className="flex items-center justify-between gap-3">
              <span className="text-purple-400">Network</span>
              <span className="font-mono text-purple-100">{network}</span>
            </div>
          )}
        </div>

        {showSecondary && (
          <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-3 text-left text-[11px] text-amber-200">
            <p className="font-semibold">Taking longer than usual.</p>
            <p className="mt-1 text-amber-200/80">
              The daemon may still be starting up, or the configured network
              may be offline. On testnet, seed nodes are periodically rotated
              and can be incompatible with a newer wallet build until they're
              upgraded. You can keep waiting, point the wallet at a different
              daemon, or lock the wallet and come back later.
            </p>
          </div>
        )}

        <div className="flex flex-col gap-2 sm:flex-row">
          <button
            onClick={() => navigate("/settings")}
            className="btn btn-secondary flex-1"
          >
            <Settings className="h-4 w-4" />
            Change daemon
          </button>
          <button
            onClick={() => {
              void lockWallet();
            }}
            className="btn btn-ghost flex-1"
          >
            <Lock className="h-4 w-4" />
            Lock wallet
          </button>
        </div>
      </div>
    </div>
  );
}
