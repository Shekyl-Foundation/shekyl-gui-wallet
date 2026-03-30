import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatSkl } from "../lib/format";
import type { Balance } from "../types/daemon";

export default function BalanceCard() {
  const [balance, setBalance] = useState<Balance | null>(null);

  useEffect(() => {
    invoke<Balance>("get_balance").then(setBalance).catch(() => {});
  }, []);

  const total = balance ? formatSkl(balance.total) : "—";
  const unlocked = balance ? formatSkl(balance.unlocked) : "—";
  const staked = balance ? formatSkl(balance.staked) : "—";

  return (
    <div className="card space-y-4">
      <div className="flex items-center gap-3">
        <img
          src="/assets/shekyl_symbol.svg"
          alt="SKL"
          className="h-8 w-8"
        />
        <div>
          <p className="text-xs text-purple-300">Total Balance</p>
          <p className="text-2xl font-bold text-gold-400">{total} SKL</p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 border-t border-purple-700/50 pt-4">
        <div>
          <p className="text-xs text-purple-300">Unlocked</p>
          <p className="text-sm font-semibold text-white">{unlocked} SKL</p>
        </div>
        <div>
          <p className="text-xs text-purple-300">Staked</p>
          <p className="text-sm font-semibold text-white">{staked} SKL</p>
        </div>
      </div>
    </div>
  );
}
