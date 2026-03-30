import { ShieldCheck } from "lucide-react";

export default function LoadingScreen() {
  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-6 bg-purple-900">
      <div className="animate-pulse">
        <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-gold-500/20">
          <ShieldCheck className="h-8 w-8 text-gold-400" />
        </div>
      </div>
      <div className="space-y-2 text-center">
        <h1 className="text-lg font-bold text-white">Shekyl Wallet</h1>
        <p className="text-xs text-purple-300">Starting wallet service...</p>
      </div>
    </div>
  );
}
