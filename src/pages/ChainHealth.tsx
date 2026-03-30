import ChainHealthPanel from "../components/ChainHealthPanel";

export default function ChainHealthPage() {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <h1 className="text-xl font-bold text-white">Chain Health</h1>
      <ChainHealthPanel />
    </div>
  );
}
