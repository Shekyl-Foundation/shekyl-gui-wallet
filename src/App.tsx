import { Routes, Route } from "react-router-dom";
import { WalletProvider } from "./context/WalletContext";
import { DaemonProvider } from "./context/DaemonContext";
import { useWallet } from "./context/useWallet";
import Layout from "./components/Layout";
import Dashboard from "./pages/Dashboard";
import Send from "./pages/Send";
import Receive from "./pages/Receive";
import Mining from "./pages/Mining";
import Staking from "./pages/Staking";
import Transactions from "./pages/Transactions";
import Settings from "./pages/Settings";
import ChainHealthPage from "./pages/ChainHealth";
import Help from "./pages/Help";
import Welcome from "./pages/Welcome";
import CreateWallet from "./pages/CreateWallet";
import ImportWallet from "./pages/ImportWallet";
import Unlock from "./pages/Unlock";
import LoadingScreen from "./components/LoadingScreen";

function App() {
  return (
    <WalletProvider>
      <WalletGate />
    </WalletProvider>
  );
}

function WalletGate() {
  const { phase } = useWallet();

  switch (phase) {
    case "loading":
      return <LoadingScreen />;

    case "no_wallet":
    case "creating":
    case "importing":
      return (
        <Routes>
          <Route index element={<Welcome />} />
          <Route path="create" element={<CreateWallet />} />
          <Route path="import" element={<ImportWallet />} />
          <Route path="*" element={<Welcome />} />
        </Routes>
      );

    case "unlock":
    case "select_wallet":
      return (
        <Routes>
          <Route index element={<Unlock />} />
          <Route path="import" element={<ImportWallet />} />
          <Route path="*" element={<Unlock />} />
        </Routes>
      );

    case "ready":
      return (
        <DaemonProvider>
          <Routes>
            <Route element={<Layout />}>
              <Route index element={<Dashboard />} />
              <Route path="send" element={<Send />} />
              <Route path="receive" element={<Receive />} />
              <Route path="mining" element={<Mining />} />
              <Route path="staking" element={<Staking />} />
              <Route path="transactions" element={<Transactions />} />
              <Route path="settings" element={<Settings />} />
              <Route path="chain-health" element={<ChainHealthPage />} />
              <Route path="help" element={<Help />} />
            </Route>
          </Routes>
        </DaemonProvider>
      );
  }
}

export default App;
