import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { WalletProvider } from "../WalletContext";
import { useWallet } from "../useWallet";
import PreWalletNode from "../../components/PreWalletNode";
import { MemoryRouter } from "react-router";
import { WalletGate } from "../../App";

// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

/** Each network's folder and the wallets in it, as the backend answers. */
const FOLDERS: Record<string, { dir: string; files: string[] }> = {
  mainnet: { dir: "/root/wallets/mainnet", files: ["main_one"] },
  testnet: { dir: "/root/wallets/testnet", files: [] },
  stagenet: { dir: "/root/wallets/stagenet", files: ["stage_one", "stage_two"] },
};

/** The phase, the folder and its wallets, as WalletGate would read them. */
function Probe() {
  const { phase, walletDir, walletFiles } = useWallet();
  return (
    <div>
      <span data-testid="phase">{phase}</span>
      <span data-testid="dir">{walletDir}</span>
      <span data-testid="files">{walletFiles.map((f) => f.name).join(",")}</span>
    </div>
  );
}

let network = "mainnet";

beforeEach(() => {
  network = "mainnet";
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case "check_wallet_dir":
        return undefined;
      case "get_wallet_dir":
        return { dir: FOLDERS[network].dir };
      case "check_wallet_files":
        return FOLDERS[network].files.map((name) => ({ name, path: "", modified: 0 }));
      case "daemon_connection_disclosures":
        return { url: null, warnings: [], network };
      case "set_daemon_connection":
        network = (args as { network: string }).network;
        return { url: null, warnings: [], network };
      case "daemon_status":
        return null;
      default:
        return null;
    }
  });
});

describe("a change of network before a wallet is open", () => {
  /**
   * Each network keeps its wallets in its own folder (Rick, 2026-11-20), so
   * a switch reads the new network's folder, lists its wallets, and moves to
   * the phase they call for. Red edit: drop `networkChanged()` from
   * PreWalletNode's switch, and the list stays the first network's.
   */
  it("lists the new network's wallets from its own folder", async () => {
    render(
      <WalletProvider>
        <PreWalletNode />
        <Probe />
      </WalletProvider>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unlock"));
    expect(screen.getByTestId("dir")).toHaveTextContent("/root/wallets/mainnet");
    expect(screen.getByTestId("files")).toHaveTextContent("main_one");

    await userEvent.click(await screen.findByRole("button", { name: "testnet" }));
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("no_wallet"));
    expect(screen.getByTestId("dir")).toHaveTextContent("/root/wallets/testnet");
    expect(screen.getByTestId("files")).toBeEmptyDOMElement();

    await userEvent.click(screen.getByRole("button", { name: "stagenet" }));
    await waitFor(() =>
      expect(screen.getByTestId("phase")).toHaveTextContent("select_wallet"),
    );
    expect(screen.getByTestId("dir")).toHaveTextContent("/root/wallets/stagenet");
    expect(screen.getByTestId("files")).toHaveTextContent("stage_one,stage_two");
  });

  /**
   * A create form under way keeps what was typed when a network switch
   * finds wallets where there were none: every pre-wallet phase shares one
   * route tree, so the phase changes from no wallet to unlock without
   * remounting the form (finding on #81). Red edit: give the no-wallet
   * phase its own `<PreWalletShell>` tree again, and the form is remounted
   * empty.
   */
  it("keeps a create form's entries across a switch that finds wallets", async () => {
    network = "testnet";
    render(
      <MemoryRouter initialEntries={["/create"]}>
        <WalletProvider>
          <WalletGate />
        </WalletProvider>
      </MemoryRouter>,
    );
    const name = await screen.findByPlaceholderText("My Wallet");
    await userEvent.clear(name);
    await userEvent.type(name, "Half Typed");

    await userEvent.click(await screen.findByRole("button", { name: "mainnet" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "mainnet" })).toHaveAttribute(
        "aria-pressed",
        "true",
      ),
    );
    // The start's list and the switch's: the phase has moved to unlock.
    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "check_wallet_files"),
      ).toHaveLength(2),
    );
    await act(async () => {});
    expect(screen.getByPlaceholderText("My Wallet")).toHaveValue("Half Typed");
  });

  /**
   * After a switch to a network whose folder holds other wallets, Unlock
   * names and opens the new folder's wallet, never the old network's name,
   * through the real gate and provider (finding on #81, fixed by #86). Red
   * edit: keep the stale pick in Unlock, and the open goes nowhere.
   */
  it("unlocks the new network's wallet after a switch", async () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <WalletProvider>
          <WalletGate />
        </WalletProvider>
      </MemoryRouter>,
    );
    await screen.findByPlaceholderText(/enter your password/i);
    expect(screen.getByText("main_one")).toBeInTheDocument();

    await userEvent.click(await screen.findByRole("button", { name: "stagenet" }));
    expect(await screen.findByRole("button", { name: "stage_one" })).toBeInTheDocument();
    expect(screen.queryByText("main_one")).not.toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText(/enter your password/i), "pw-for-test");
    await userEvent.click(screen.getByRole("button", { name: /^unlock$/i }));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("open_wallet", {
        filename: "stage_one",
        password: "pw-for-test",
      }),
    );
  });
});
