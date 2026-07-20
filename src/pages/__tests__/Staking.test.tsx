import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import { WalletContext } from "../../context/walletState";
import type { WalletContextValue } from "../../context/walletState";
import Staking from "../Staking";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_chain_health" || cmd === "get_wallet_status") {
      return null;
    }
    if (
      cmd === "list_shard_preview_fixtures" ||
      cmd === "list_shards"
    ) {
      return [];
    }
    if (cmd === "get_staker_status") {
      return {
        staking_enabled: false,
        has_stake_engine: false,
        bonded_slot_count: 0,
        has_pscan: false,
      };
    }
    return null;
  });
});

const walletStub: WalletContextValue = {
  phase: "select_wallet",
  walletFiles: [],
  walletName: null,
  walletAddress: null,
  rpcReady: false,
  error: null,
  openWallet: () => Promise.reject("stub"),
  createWallet: () => Promise.reject("stub"),
  importFromSeed: () => Promise.reject("stub"),
  importFromKeys: () => Promise.reject("stub"),
  lockWallet: () => Promise.resolve(),
  setPhase: () => {},
  refreshFiles: async () => [],
  walletDir: null,
  walletDirFallbackFrom: null,
  setCustomWalletDir: async () => "",
  resetWalletDir: async () => "",
  refreshWalletDir: async () => "",
};

function renderStaking(wallet: Partial<WalletContextValue> = {}) {
  return render(
    <WalletContext.Provider value={{ ...walletStub, ...wallet }}>
      <DaemonProvider>
        <Staking />
      </DaemonProvider>
    </WalletContext.Provider>,
  );
}

describe("Staking (archival activation)", () => {
  it("renders archival model without claim-era actions", () => {
    renderStaking();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(screen.getByText("Archival staking")).toBeInTheDocument();
    expect(screen.getByText("Become a staker")).toBeInTheDocument();
    expect(screen.queryByText(/Select Staking Tier/)).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/Amount to stake/)).not.toBeInTheDocument();
  });

  it("prompts to open a wallet when not ready", () => {
    renderStaking({ phase: "select_wallet" });
    expect(
      screen.getByText(/Open an Engine wallet to activate staking/),
    ).toBeInTheDocument();
  });

  it("shows password activation when wallet is ready on Engine", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (
        cmd === "list_shard_preview_fixtures" ||
        cmd === "list_shards"
      ) {
        return [];
      }
      if (cmd === "get_staker_status") {
        return {
          staking_enabled: false,
          has_stake_engine: false,
          bonded_slot_count: 0,
          has_pscan: false,
        };
      }
      return null;
    });
    renderStaking({ phase: "ready", walletName: "alice" });
    expect(
      await screen.findByPlaceholderText("Wallet password"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Activate staker/i })).toBeInTheDocument();
  });
});
