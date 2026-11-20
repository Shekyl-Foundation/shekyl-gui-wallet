import type { ReactElement } from "react";
import { Routes, Route } from "react-router";
import { WalletProvider } from "./context/WalletContext";
import { DaemonProvider } from "./context/DaemonContext";
import { TorProvider } from "./context/TorContext";
import { ShardPickerProvider } from "./context/ShardPickerContext";
import { useWallet } from "./context/useWallet";
import Layout from "./components/Layout";
import Dashboard from "./pages/Dashboard";
import Send from "./pages/Send";
import Receive from "./pages/Receive";
import Mining from "./pages/Mining";
import Staking from "./pages/Staking";
import Shards from "./pages/Shards";
import Transactions from "./pages/Transactions";
import Settings from "./pages/Settings";
import ChainHealthPage from "./pages/ChainHealth";
import Help from "./pages/Help";
import Proofs from "./pages/Proofs";
import Sign from "./pages/Sign";
import MultisigRoute from "./components/multisig/MultisigRoute";
import Welcome from "./pages/Welcome";
import CreateWallet from "./pages/CreateWallet";
import ImportWallet from "./pages/ImportWallet";
import Unlock from "./pages/Unlock";
import { OpenAttemptProvider } from "./pages/OpenAttemptProvider";
import LoadingScreen from "./components/LoadingScreen";
import PreWalletShell from "./components/PreWalletShell";

function App() {
  return (
    <WalletProvider>
      <WalletGate />
    </WalletProvider>
  );
}

/**
 * The routes before a wallet is open, one tree for every pre-wallet phase:
 * the phase chooses only the resting page (`Welcome` with no wallet,
 * `Unlock` with one or more). A change of phase, such as a network switch
 * that finds wallets where there were none, keeps the tree and so keeps a
 * create or import form under way with what was typed in it. The attempt
 * lives on this shell: create is a child route, so a refusal survives the
 * trip to the create screen and back, and it stays tied to its file.
 */
function PreWalletRoutes({ resting }: { resting: ReactElement }) {
  return (
    <OpenAttemptProvider>
      <PreWalletShell>
        <Routes>
          <Route index element={resting} />
          <Route path="create" element={<CreateWallet />} />
          <Route path="import" element={<ImportWallet />} />
          <Route path="*" element={resting} />
        </Routes>
      </PreWalletShell>
    </OpenAttemptProvider>
  );
}

export function WalletGate() {
  const { phase } = useWallet();

  switch (phase) {
    case "loading":
      return <LoadingScreen />;

    case "no_wallet":
      return <PreWalletRoutes resting={<Welcome />} />;

    case "unlock":
    case "select_wallet":
      return <PreWalletRoutes resting={<Unlock />} />;

    case "ready":
      return (
        <DaemonProvider>
          <TorProvider>
            <ShardPickerProvider>
              <Routes>
                <Route element={<Layout />}>
                  <Route index element={<Dashboard />} />
                  <Route path="send" element={<Send />} />
                  <Route path="receive" element={<Receive />} />
                  <Route path="mining" element={<Mining />} />
                  <Route path="staking" element={<Staking />} />
                  <Route path="shards" element={<Shards />} />
                  <Route path="transactions" element={<Transactions />} />
                  <Route path="proofs" element={<Proofs />} />
                  <Route path="sign" element={<Sign />} />
                  <Route path="settings" element={<Settings />} />
                  <Route path="chain-health" element={<ChainHealthPage />} />
                  <Route path="multisig" element={<MultisigRoute />} />
                  <Route path="help" element={<Help />} />
                  {/* Catch-all so stale URLs from the no-wallet/unlock phases
                    (e.g. /create, /import) never render an empty tree. */}
                  <Route path="*" element={<Dashboard />} />
                </Route>
              </Routes>
            </ShardPickerProvider>
          </TorProvider>
        </DaemonProvider>
      );
  }
}

export default App;
