import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Link } from "react-router-dom";
import { Send, Download, Coins, ArrowLeftRight } from "lucide-react";
import BalanceCard from "../components/BalanceCard";

interface WalletStatus {
  connected: boolean;
  wallet_open: boolean;
}

export default function Dashboard() {
  const [status, setStatus] = useState<WalletStatus | null>(null);

  useEffect(() => {
    invoke<WalletStatus>("get_wallet_status").then(setStatus).catch(() => {});
  }, []);

  if (!status?.wallet_open) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-6 text-center">
        <img
          src="/assets/shekyl_icon_v2.svg"
          alt="Shekyl"
          className="h-20 w-20 opacity-60"
        />
        <div>
          <h1 className="text-2xl font-bold text-white">Welcome to Shekyl Wallet</h1>
          <p className="mt-2 text-purple-300">
            Create a new wallet or open an existing one to get started.
          </p>
        </div>
        <div className="flex gap-3">
          <button className="btn btn-primary">Create Wallet</button>
          <button className="btn btn-secondary">Open Wallet</button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-bold text-white">Dashboard</h1>

      <BalanceCard />

      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <QuickAction to="/send" icon={Send} label="Send" />
        <QuickAction to="/receive" icon={Download} label="Receive" />
        <QuickAction to="/staking" icon={Coins} label="Staking" />
        <QuickAction to="/transactions" icon={ArrowLeftRight} label="History" />
      </div>
    </div>
  );
}

function QuickAction({
  to,
  icon: Icon,
  label,
}: {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
}) {
  return (
    <Link
      to={to}
      className="card flex flex-col items-center gap-2 py-6 text-center transition-colors hover:border-gold-500/50"
    >
      <Icon className="h-6 w-6 text-gold-400" />
      <span className="text-sm font-medium text-purple-200">{label}</span>
    </Link>
  );
}
