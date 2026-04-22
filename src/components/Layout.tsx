import { Outlet, useLocation } from "react-router-dom";
import Sidebar from "./Sidebar";
import Header from "./Header";
import TestnetBanner from "./TestnetBanner";
import DaemonOfflineBanner from "./DaemonOfflineBanner";
import DaemonConnectingScreen from "./DaemonConnectingScreen";
import { useDaemon } from "../context/useDaemon";

// Routes that render even before the daemon is reachable. Settings must be
// reachable so a misconfigured daemon URL can be fixed; Help is reachable
// because it's never daemon-dependent.
const DAEMON_BYPASS_PREFIXES = ["/settings", "/help"];

export default function Layout() {
  const { hasEverConnected } = useDaemon();
  const { pathname } = useLocation();

  const bypass = DAEMON_BYPASS_PREFIXES.some((p) => pathname.startsWith(p));
  const showConnecting = !hasEverConnected && !bypass;

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Sidebar />
      <div className="flex flex-1 flex-col overflow-hidden">
        <DaemonOfflineBanner />
        <TestnetBanner />
        <Header />
        <main className="flex-1 overflow-y-auto p-6">
          {showConnecting ? <DaemonConnectingScreen /> : <Outlet />}
        </main>
      </div>
    </div>
  );
}
