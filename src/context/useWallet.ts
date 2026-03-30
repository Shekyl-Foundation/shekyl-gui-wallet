import { useContext } from "react";
import { WalletContext } from "./walletState";

export function useWallet() {
  return useContext(WalletContext);
}
