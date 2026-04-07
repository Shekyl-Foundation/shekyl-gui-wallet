import { Link, useNavigate } from "react-router-dom";
import { Send, Download, Coins, ArrowLeftRight, ShieldCheck } from "lucide-react";
import BalanceCard from "../components/BalanceCard";
import ChainHealthPanel from "../components/ChainHealthPanel";
import { useDaemon } from "../context/useDaemon";

export default function Dashboard() {
  const { health, security } = useDaemon();
  const navigate = useNavigate();

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-bold text-white">Dashboard</h1>

      <BalanceCard />

      {security && security.anonymity_set_size > 0 && (
        <button
          onClick={() => navigate("/settings")}
          className="flex w-full items-center justify-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-300 transition-colors hover:bg-emerald-500/10"
        >
          <ShieldCheck className="h-3 w-3" />
          3 layers active — {security.anonymity_set_size.toLocaleString()} output anonymity set
        </button>
      )}

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
