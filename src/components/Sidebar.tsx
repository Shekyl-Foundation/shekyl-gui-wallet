import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Send,
  Download,
  Coins,
  Pickaxe,
  ArrowLeftRight,
  Activity,
  HelpCircle,
  Settings,
} from "lucide-react";

const links = [
  { to: "/", icon: LayoutDashboard, label: "Dashboard" },
  { to: "/send", icon: Send, label: "Send" },
  { to: "/receive", icon: Download, label: "Receive" },
  { to: "/mining", icon: Pickaxe, label: "Mining" },
  { to: "/staking", icon: Coins, label: "Staking" },
  { to: "/transactions", icon: ArrowLeftRight, label: "Transactions" },
  { to: "/chain-health", icon: Activity, label: "Chain Health" },
  { to: "/help", icon: HelpCircle, label: "Help" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

export default function Sidebar() {
  return (
    <aside className="flex w-56 flex-col border-r border-purple-700/50 bg-purple-900/80">
      {/* Logo */}
      <div className="flex items-center gap-3 px-5 py-5">
        <img
          src="/assets/shekyl_icon_v2.svg"
          alt="Shekyl"
          className="h-8 w-8"
        />
        <span className="text-lg font-bold text-gold-400">Shekyl</span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-1 px-3 py-2">
        {links.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            end={to === "/"}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-gold-500/15 text-gold-400"
                  : "text-purple-200 hover:bg-white/5 hover:text-white"
              }`
            }
          >
            <Icon className="h-4 w-4 shrink-0" />
            {label}
          </NavLink>
        ))}
      </nav>

      {/* Version */}
      <div className="border-t border-purple-700/50 px-5 py-3">
        <p className="text-xs text-purple-400">Shekyl Wallet v0.1.0-beta</p>
      </div>
    </aside>
  );
}
