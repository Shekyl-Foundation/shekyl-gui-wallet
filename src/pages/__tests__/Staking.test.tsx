import { render, screen, waitFor } from "@testing-library/react";
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

// DS-PR-3 PR-B: drainable-P read on the active-staker panel. The load-bearing
// contract (rule 82; DS-PR-3 locked decision) is that a balance read never
// renders a fabricated zero — the transient "syncing" arm and a non-transient
// fault both render a non-value ("Syncing…" / "—"), distinct from a real 0.
describe("Staking drainable-P (DS-PR-3 PR-B)", () => {
  function mockStakerWithDrain(drain: unknown | (() => Promise<never>)) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_shard_preview_fixtures" || cmd === "list_shards") {
        return [];
      }
      if (cmd === "get_staker_status") {
        return {
          staking_enabled: true,
          has_stake_engine: true,
          bonded_slot_count: 1,
          has_pscan: true,
        };
      }
      if (cmd === "get_drain_balance") {
        return typeof drain === "function"
          ? (drain as () => Promise<never>)()
          : drain;
      }
      return null;
    });
  }

  it("renders the anchored drainable figure for an active staker", async () => {
    mockStakerWithDrain({ status: "ready", spendable: 1_500_000_000 });
    renderStaking({ phase: "ready", walletName: "alice" });
    const line = await screen.findByText(/Drainable \(P\)/);
    await waitFor(() => expect(line.textContent).toContain("1.500000 SKL"));
  });

  it("shows 'Syncing…' (no value) while the reference is unanchorable", async () => {
    mockStakerWithDrain({ status: "syncing", detail: "still ingesting blocks" });
    renderStaking({ phase: "ready", walletName: "alice" });
    expect(await screen.findByText("Syncing…")).toBeInTheDocument();
    // No SKL value rendered ⇒ syncing is never conflated with a real balance.
    const line = screen.getByText(/Drainable \(P\)/);
    expect(line.textContent).not.toContain("SKL");
  });

  it("renders '—' (never a fabricated zero) when the read faults", async () => {
    mockStakerWithDrain(() => Promise.reject("read fault"));
    renderStaking({ phase: "ready", walletName: "alice" });
    const line = await screen.findByText(/Drainable \(P\)/);
    await waitFor(() => expect(line.textContent).toContain("—"));
    // A fault renders a dash, not a value ⇒ no fabricated zero.
    expect(line.textContent).not.toContain("SKL");
  });
});

// GUI-PR3b: the "Your stake" panel projects Engine::staking_read_view
// (WI-RPC-1). Contracts under test: the three balance legs render as
// distinct figures (never summed), and a read fault renders as an explicit
// fault message — never an empty/zero panel over a bad seal (rule 82,
// mirroring the core's fail-closed StakingReadError).
describe("Staking view panel (GUI-PR3b)", () => {
  function mockStakerWithView(view: unknown | (() => Promise<never>)) {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_shard_preview_fixtures" || cmd === "list_shards") {
        return [];
      }
      if (cmd === "get_staker_status") {
        return {
          staking_enabled: true,
          has_stake_engine: true,
          bonded_slot_count: 1,
          has_pscan: true,
        };
      }
      if (cmd === "get_drain_balance") {
        return { status: "ready", spendable: 0 };
      }
      if (cmd === "get_staking_view") {
        return typeof view === "function"
          ? (view as () => Promise<never>)()
          : view;
      }
      return null;
    });
  }

  it("renders the three balance legs distinctly and the output rows", async () => {
    mockStakerWithView({
      staking_enabled: true,
      bonded_principal_confirmed: 1_000_000_000,
      bonded_principal_pending: 2_000_000_000,
      rewards_received_unspent: 3_000_000_000,
      staked_outputs: [
        {
          gindex: 42,
          amount: 4_000_000_000,
          p_slot: 3,
          unlock_height: 12345,
          confirmed: true,
        },
      ],
      pscan_synced_height: 99000,
    });
    renderStaking({ phase: "ready", walletName: "alice" });

    expect(await screen.findByText("Your stake")).toBeInTheDocument();
    // Three legs, three distinct figures — never a single summed number.
    expect(screen.getByText("Bonded (confirmed)")).toBeInTheDocument();
    expect(screen.getByText("1.000000")).toBeInTheDocument();
    expect(screen.getByText("Bonded (pending)")).toBeInTheDocument();
    expect(screen.getByText("2.000000")).toBeInTheDocument();
    expect(screen.getByText("Rewards (unspent)")).toBeInTheDocument();
    expect(screen.getByText("3.000000")).toBeInTheDocument();
    // Output row: slot, amount, unlock height.
    expect(screen.getByText("Staked outputs (1)")).toBeInTheDocument();
    expect(screen.getByText("Slot 3")).toBeInTheDocument();
    expect(screen.getByText("4.000000 SKL")).toBeInTheDocument();
    expect(screen.getByText(/unlocks at 12,345/)).toBeInTheDocument();
    expect(
      screen.getByText(/Persona scan synced to block 99,000/),
    ).toBeInTheDocument();
  });

  it("shows an honest empty state when a staker has no staked outputs", async () => {
    mockStakerWithView({
      staking_enabled: true,
      bonded_principal_confirmed: 1_000_000_000,
      bonded_principal_pending: 0,
      rewards_received_unspent: 0,
      staked_outputs: [],
      pscan_synced_height: null,
    });
    renderStaking({ phase: "ready", walletName: "alice" });

    expect(
      await screen.findByText(/No staked outputs yet/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Persona scan has not sealed a frontier yet/),
    ).toBeInTheDocument();
  });

  it("renders a fault message (never an empty/zero panel) when the read faults", async () => {
    mockStakerWithView(() => Promise.reject("seal failed to open"));
    renderStaking({ phase: "ready", walletName: "alice" });

    expect(
      await screen.findByText(/Staking state could not be read/),
    ).toBeInTheDocument();
    // Fail-closed: no balance legs and no fabricated empty-output state.
    expect(screen.queryByText("Bonded (confirmed)")).not.toBeInTheDocument();
    expect(screen.queryByText(/No staked outputs yet/)).not.toBeInTheDocument();
  });
});
