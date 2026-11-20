import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { WalletGate } from "../../App";

const { openWallet, setPhase, files } = vi.hoisted(() => ({
  openWallet: vi.fn(),
  setPhase: vi.fn(),
  files: {
    current: [{ name: "alpha11", path: "/w/alpha11.wallet.keys" }] as {
      name: string;
      path: string;
    }[],
  },
}));

vi.mock("../../context/useWallet", () => ({
  useWallet: () => ({
    phase: "unlock" as const,
    walletFiles: files.current,
    openWallet,
    createWallet: vi.fn(),
    setPhase,
    error: null,
  }),
}));
// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));
// Create's leave path asks Rust to clear the clipboard. These tests never leave the page's own concern.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

const EARLIER = "earlier-release sentence from open_wallet";
const CURRENT = { name: "current", path: "/w/current.wallet.keys" };
const ALPHA11 = { name: "alpha11", path: "/w/alpha11.wallet.keys" };

beforeEach(() => {
  openWallet.mockReset();
  setPhase.mockReset();
  files.current = [ALPHA11];
});

function renderGate() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <WalletGate />
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
    renderGate();
    unlock();

    expect(await screen.findByText(EARLIER)).toBeInTheDocument();
    expect(screen.queryByText(/forgot your password/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /create a new wallet/i }));
    expect(
      await screen.findByRole("heading", { name: /create new wallet/i }),
    ).toBeInTheDocument();
    expect(setPhase).not.toHaveBeenCalled();

    const back = document.querySelector("svg.lucide-arrow-left")?.closest("button");
    if (!(back instanceof HTMLButtonElement)) {
      throw new Error("create screen has no back button");
    }
    fireEvent.click(back);

    expect(
      await screen.findByRole("heading", { name: /unlock wallet/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(EARLIER)).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /shekyl wallet/i })).not.toBeInTheDocument();
  });

  it("shows a wrong password's sentence and keeps the restore hint", async () => {
    openWallet.mockRejectedValue({
      kind: "wrong_password",
      message: "Incorrect password. Please try again.",
    });
    renderGate();
    unlock();

    expect(
      await screen.findByText("Incorrect password. Please try again."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /create a new wallet/i })).not.toBeInTheDocument();
    expect(screen.getByText(/forgot your password/i)).toBeInTheDocument();
  });

  it("shows any other refusal's sentence as it came, without a new-wallet button", async () => {
    openWallet.mockRejectedValue({
      kind: "other",
      message: "the wallet is open in another process",
    });
    renderGate();
    unlock();

    expect(await screen.findByText("the wallet is open in another process")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /create a new wallet/i })).not.toBeInTheDocument();
    expect(screen.getByText(/forgot your password/i)).toBeInTheDocument();
  });

  it("keeps the message of an error object that has no kind", async () => {
    openWallet.mockRejectedValue({ message: "the connection to the backend was lost" });
    renderGate();
    unlock();

    expect(await screen.findByText("the connection to the backend was lost")).toBeInTheDocument();
    expect(screen.queryByText("[object Object]")).not.toBeInTheDocument();
  });

  it("does not take a message that mentions a password for a wrong password", async () => {
    openWallet.mockRejectedValue("the password file could not be read");
    renderGate();
    unlock();

    expect(await screen.findByText("the password file could not be read")).toBeInTheDocument();
    expect(screen.queryByText("Incorrect password. Please try again.")).not.toBeInTheDocument();
  });

  it("shows an earlier-release refusal only for the file that was opened", async () => {
    files.current = [ALPHA11, CURRENT];
    openWallet.mockRejectedValue({ kind: "earlier_release", message: EARLIER });
    renderGate();
    unlock();

    expect(await screen.findByText(EARLIER)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "alpha11" }));
    fireEvent.click(screen.getByRole("button", { name: "current" }));

    expect(screen.queryByText(EARLIER)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /create a new wallet/i })).not.toBeInTheDocument();
    expect(screen.getByText(/forgot your password/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "current" }));
    fireEvent.click(screen.getByRole("button", { name: "alpha11" }));

    expect(screen.getByText(EARLIER)).toBeInTheDocument();
    expect(screen.queryByText(/forgot your password/i)).not.toBeInTheDocument();
  });

  it("applies a late refusal to the file that was opened, not the one now selected", async () => {
    files.current = [ALPHA11, CURRENT];
    let rejectOpen: (reason: unknown) => void = () => {};
    openWallet.mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectOpen = reject;
        }),
    );
    renderGate();
    unlock();

    fireEvent.click(screen.getByRole("button", { name: "alpha11" }));
    fireEvent.click(screen.getByRole("button", { name: "current" }));
    await act(async () => {
      rejectOpen({ kind: "earlier_release", message: EARLIER });
    });

    expect(screen.queryByText(EARLIER)).not.toBeInTheDocument();
    expect(screen.getByText(/forgot your password/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "current" }));
    fireEvent.click(screen.getByRole("button", { name: "alpha11" }));
    expect(screen.getByText(EARLIER)).toBeInTheDocument();
  });
});
