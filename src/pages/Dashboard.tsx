import { Link } from "react-router-dom";
import { Send, Download, Coins, ArrowLeftRight } from "lucide-react";
import BalanceCard from "../components/BalanceCard";
import ChainHealthPanel from "../components/ChainHealthPanel";
import { useDaemon } from "../context/useDaemon";

export default function Dashboard() {
  const { health } = useDaemon();

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

      {health && <ChainHealthPanel compact />}
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
