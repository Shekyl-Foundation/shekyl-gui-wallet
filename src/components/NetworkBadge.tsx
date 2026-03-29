interface Props {
  network: string;
}

const colors: Record<string, string> = {
  mainnet: "bg-emerald-500/20 text-emerald-300 border-emerald-500/30",
  testnet: "bg-amber-500/20 text-amber-300 border-amber-500/30",
  stagenet: "bg-blue-500/20 text-blue-300 border-blue-500/30",
};

export default function NetworkBadge({ network }: Props) {
  const cls = colors[network] ?? colors.mainnet;
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider ${cls}`}
    >
      {network}
    </span>
  );
}
