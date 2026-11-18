import type { ReactNode } from "react";
import PreWalletNode from "./PreWalletNode";

/**
 * The window before a wallet is open: the node's bar, and under it the
 * page, in the height that is left. The page scrolls inside that height,
 * so a long notice in the bar never pushes a page's controls (the seed
 * confirmation, the open button) out of a window that itself does not
 * scroll.
 */
export default function PreWalletShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-purple-900">
      <div className="max-h-[50%] shrink-0 overflow-y-auto">
        <PreWalletNode />
      </div>
      <div data-testid="pre-wallet-page" className="min-h-0 flex-1 overflow-y-auto">
        {children}
      </div>
    </div>
  );
}
