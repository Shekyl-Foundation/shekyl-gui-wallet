import { useDaemon } from "../context/useDaemon";

export default function SecurityPanel() {
  const { security } = useDaemon();

  if (!security) {
    return (
      <div className="card text-center text-sm text-purple-300">
        Connect to daemon to view security status
      </div>
    );
  }

  const layers = [
    {
      index: 1,
      label: "Membership proof",
      value:
        security.anonymity_set_size > 0
          ? `FCMP++ (${security.anonymity_set_size.toLocaleString()} outputs)`
          : "FCMP++",
    },
    {
      index: 2,
      label: "Spend authorization",
      value: `${security.classical} + ${security.post_quantum.split(" ")[0]}`,
    },
    {
      index: 3,
      label: "Amount privacy",
      value: "Bulletproofs+",
    },
  ];

  const pathsLabel = security.paths_precomputed
    ? "paths ready"
    : "paths warming up";

  return (
    <div className="card space-y-3">
      {layers.map(({ index, label, value }) => (
        <div
          key={index}
          className="flex items-center justify-between text-xs"
        >
          <span className="flex items-center gap-2 text-purple-300">
            <span className="inline-flex h-4 w-4 items-center justify-center rounded-full bg-purple-700/60 text-[9px] font-bold text-purple-200">
              {index}
            </span>
            {label}
          </span>
          <span className="font-mono text-white">{value}</span>
        </div>
      ))}
      {security.tree_depth > 0 && (
        <p className="text-[10px] text-purple-400">
          Tree depth {security.tree_depth} — root{" "}
          <span className="font-mono">{security.tree_root_short}</span> —{" "}
          {pathsLabel}
        </p>
      )}
    </div>
  );
}
