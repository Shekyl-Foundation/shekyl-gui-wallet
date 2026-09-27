import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { WalletProvider } from "../WalletContext";
import { useWallet } from "../useWallet";
import ImportWallet from "../../pages/ImportWallet";

vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

const SEED = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");

/** The shape of App's WalletGate: the Import page until "ready", then the app. */
function Gate() {
  const { phase } = useWallet();
  return phase === "ready" ? <div>READY_APP</div> : <ImportWallet />;
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd) => {
    switch (cmd) {
      case "init_wallet_rpc":
        return Promise.resolve(true);
      case "get_wallet_dir":
        return Promise.resolve({ dir: "/wallets", fallback_from: null });
      case "check_wallet_files":
        return Promise.resolve([]);
      case "import_wallet_from_seed":
        return Promise.resolve({ name: "Restored_Wallet", address: "shekyl1test" });
      default:
        return Promise.reject(new Error(`unrouted invoke ${String(cmd)}`));
    }
  });
});

describe("restore through the real provider", () => {
  it("shows completion on the Import page, then hands off to the ready app", async () => {
    render(
      <MemoryRouter initialEntries={["/import"]}>
        <WalletProvider>
          <Gate />
        </WalletProvider>
      </MemoryRouter>,
    );
    await screen.findByText("24-Word Recovery Phrase");
    fireEvent.change(screen.getByPlaceholderText(/24-word recovery phrase/i), { target: { value: SEED } });
    fireEvent.change(screen.getByPlaceholderText(/at least 8 characters/i), {
      target: { value: "correct horse battery" },
    });
    fireEvent.click(screen.getByRole("button", { name: /restore from recovery phrase/i }));

    // The provider must not flip the phase itself: the page is still mounted
    // to show completion after the engine has restored the wallet.
    expect(await screen.findByText("Restore complete")).toBeInTheDocument();
    expect(screen.queryByText("READY_APP")).not.toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("import_wallet_from_seed", {
      name: "Restored Wallet",
      seed: SEED,
      password: "correct horse battery",
      restoreHeight: 0,
    });

    // …and then the page navigates off /import and flips to ready.
    await waitFor(() => expect(screen.getByText("READY_APP")).toBeInTheDocument(), { timeout: 3000 });
  });
});
