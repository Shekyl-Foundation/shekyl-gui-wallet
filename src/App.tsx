import { Routes, Route } from "react-router-dom";
import { DaemonProvider } from "./context/DaemonContext";
import Layout from "./components/Layout";
import Dashboard from "./pages/Dashboard";
import Send from "./pages/Send";
import Receive from "./pages/Receive";
import Staking from "./pages/Staking";
import Transactions from "./pages/Transactions";
import Settings from "./pages/Settings";
import ChainHealthPage from "./pages/ChainHealth";

function App() {
  return (
    <DaemonProvider>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="send" element={<Send />} />
          <Route path="receive" element={<Receive />} />
          <Route path="staking" element={<Staking />} />
          <Route path="transactions" element={<Transactions />} />
          <Route path="settings" element={<Settings />} />
          <Route path="chain-health" element={<ChainHealthPage />} />
        </Route>
      </Routes>
    </DaemonProvider>
  );
}

export default App;
