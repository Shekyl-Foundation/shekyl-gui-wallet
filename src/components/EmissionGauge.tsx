interface Props {
  value: number;
  max?: number;
  label: string;
  display: string;
  size?: number;
}

export default function EmissionGauge({
  value,
  max = 100,
  label,
  display,
  size = 80,
}: Props) {
  const pct = Math.min((value / max) * 100, 100);
  const radius = (size - 8) / 2;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (pct / 100) * circumference;

  return (
    <div className="flex flex-col items-center gap-1">
      <svg width={size} height={size} className="-rotate-90">
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={4}
          className="text-purple-700/50"
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={4}
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          strokeLinecap="round"
          className="text-gold-400 transition-all duration-700"
        />
      </svg>
      <span className="text-sm font-bold text-white">{display}</span>
      <span className="text-[10px] text-purple-300">{label}</span>
    </div>
  );
}
