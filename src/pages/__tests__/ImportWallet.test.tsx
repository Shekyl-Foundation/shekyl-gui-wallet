import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach } from "vitest";
import ImportWallet from "../ImportWallet";

const SEED = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
const restoreWallet = vi.fn();
const setPhase = vi.fn();

vi.mock("../../context/useWallet", () => ({
  useWallet: () => ({ restoreWallet, setPhase }),
}));
// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

beforeEach(() => {
  restoreWallet.mockReset();
  restoreWallet.mockResolvedValue({ name: "Restored_Wallet", address: "shekyl1test" });
});

function renderPage() {
  return render(
    <MemoryRouter>
      <ImportWallet />
    </MemoryRouter>,
  );
}

function fill({ seed = SEED, password = "correct horse battery", height = "" } = {}) {
  fireEvent.change(screen.getByPlaceholderText(/24-word recovery phrase/i), {
    target: { value: seed },
  });
  fireEvent.change(screen.getByPlaceholderText(/at least 8 characters/i), {
    target: { value: password },
  });
  if (height) fireEvent.change(screen.getByPlaceholderText("0"), { target: { value: height } });
}

describe("ImportWallet", () => {
  it("offers exactly one restore path: the recovery phrase", () => {
    renderPage();
    expect(screen.getByText("Recovery Phrase")).toBeInTheDocument();
    expect(screen.queryByText(/private keys/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/spend key|view key/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/passphrase/i)).not.toBeInTheDocument();
  });

  it("sends the name, phrase, password, and restore height it collected", async () => {
    renderPage();
    fill({ height: "1200" });
    fireEvent.click(screen.getByRole("button", { name: /restore from recovery phrase/i }));
    expect(restoreWallet).toHaveBeenCalledWith(
      "Restored Wallet",
      SEED,
      "correct horse battery",
      1200,
    );
    expect(await screen.findByText("Restore complete")).toBeInTheDocument();
  });

  it("refuses to submit until the phrase has all 24 words", () => {
    renderPage();
    fill({ seed: SEED.split(" ").slice(0, 23).join(" ") });
    expect(screen.getByRole("button", { name: /restore from recovery phrase/i })).toBeDisabled();
    expect(screen.getByText("23/24 words")).toBeInTheDocument();
  });

  it("accepts a testnet wallet's 64-hex raw seed and sends it verbatim", () => {
    const hex = "ab".repeat(32);
    renderPage();
    fill({ seed: hex });
    expect(screen.getByText("64/64 hex characters (testnet seed)")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /restore from recovery phrase/i }));
    expect(restoreWallet).toHaveBeenCalledWith("Restored Wallet", hex, "correct horse battery", 0);
  });

  it("refuses a hex seed that is not exactly 64 characters", () => {
    renderPage();
    fill({ seed: "ab".repeat(31) });
    expect(screen.getByText("62/64 hex characters (testnet seed)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /restore from recovery phrase/i })).toBeDisabled();
  });

  it("shows the engine's refusal and returns to the form", async () => {
    restoreWallet.mockRejectedValue("invalid recovery phrase");
    renderPage();
    fill();
    fireEvent.click(screen.getByRole("button", { name: /restore from recovery phrase/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("invalid recovery phrase");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /restore from recovery phrase/i })).toBeEnabled(),
    );
  });
});
