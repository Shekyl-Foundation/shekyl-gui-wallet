import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import Unlock from "../Unlock";

const openWallet = vi.fn();
const setPhase = vi.fn();

vi.mock("../../context/useWallet", () => ({
  useWallet: () => ({
    walletFiles: [{ name: "alpha11", path: "/w/alpha11.wallet.keys" }],
    openWallet,
    setPhase,
    error: null,
  }),
}));
// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

const EARLIER =
  "This wallet file was made by an earlier release, and this release cannot open it.";

beforeEach(() => {
  openWallet.mockReset();
  setPhase.mockReset();
});

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route index element={<Unlock />} />
        <Route path="create" element={<p>create page</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

function unlock(password = "fixture-password") {
  fireEvent.change(screen.getByPlaceholderText(/enter your password/i), {
    target: { value: password },
  });
  fireEvent.click(screen.getByRole("button", { name: /^unlock$/i }));
}

describe("Unlock", () => {
  it("says an earlier release's file cannot open and offers a new wallet in one step", async () => {
    openWallet.mockRejectedValue({ kind: "earlier_release", message: EARLIER });
    renderPage();
    unlock();

    expect(await screen.findByText(EARLIER)).toBeInTheDocument();
    expect(screen.queryByText(/forgot your password/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /create a new wallet/i }));
    expect(setPhase).toHaveBeenCalledWith("no_wallet");
    expect(await screen.findByText("create page")).toBeInTheDocument();
  });

  it("reads a wrong password by its kind, not its words", async () => {
    openWallet.mockRejectedValue({ kind: "wrong_password", message: "anything" });
    renderPage();
    unlock();

    expect(await screen.findByText("Incorrect password. Please try again.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /create a new wallet/i })).not.toBeInTheDocument();
  });

  it("shows any other refusal's sentence as it came, without a new-wallet button", async () => {
    openWallet.mockRejectedValue({
      kind: "other",
      message: "the wallet is open in another process",
    });
    renderPage();
    unlock();

    expect(await screen.findByText("the wallet is open in another process")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /create a new wallet/i })).not.toBeInTheDocument();
    expect(screen.getByText(/forgot your password/i)).toBeInTheDocument();
  });

  it("does not take a message that mentions a password for a wrong password", async () => {
    openWallet.mockRejectedValue("the password file could not be read");
    renderPage();
    unlock();

    expect(await screen.findByText("the password file could not be read")).toBeInTheDocument();
  });
});
