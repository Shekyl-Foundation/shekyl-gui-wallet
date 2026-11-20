import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import CreateWallet from "../CreateWallet";

const SEED = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
/** The delay the command reports. Production owns the real constant. */
const REPORTED_CLEAR_AFTER_MS = 60_000;
const createWallet = vi.fn();
const setPhase = vi.fn();

vi.mock("../../context/useWallet", () => ({
  useWallet: () => ({ createWallet, setPhase }),
}));
// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation((cmd) => {
    if (cmd === "copy_to_clipboard") {
      return Promise.resolve({ clear_after_ms: REPORTED_CLEAR_AFTER_MS });
    }
    if (cmd === "clear_clipboard") return Promise.resolve(false);
    if (cmd === "get_primary_address") return Promise.resolve({ address: "shekyl1test" });
    return Promise.resolve(undefined);
  });
  createWallet.mockReset();
  createWallet.mockResolvedValue({
    wallet: { name: "My_Wallet", capability: "FULL", network: "MAINNET" },
    mnemonic: SEED,
  });
});

afterEach(() => {
  vi.useRealTimers();
});

async function reachSeedStep() {
  const view = render(
    <MemoryRouter>
      <CreateWallet />
    </MemoryRouter>,
  );
  fireEvent.change(screen.getByPlaceholderText("At least 8 characters"), {
    target: { value: "correct horse battery" },
  });
  fireEvent.change(screen.getByPlaceholderText("Re-enter your password"), {
    target: { value: "correct horse battery" },
  });
  fireEvent.click(screen.getByRole("button", { name: /continue/i }));
  await screen.findByText("Copy to clipboard");
  return view;
}

async function clickCopy() {
  vi.useFakeTimers();
  fireEvent.click(screen.getByText("Copy to clipboard"));
  await act(async () => {
    await Promise.resolve();
  });
  expect(vi.mocked(invoke)).toHaveBeenCalledWith("copy_to_clipboard", { text: SEED });
  expect(screen.getByRole("status")).toHaveTextContent(/60 seconds/);
}

const clearCalls = () =>
  vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "clear_clipboard").length;

describe("CreateWallet seed clipboard mitigation", () => {
  it("shows the clear delay Rust reported and does not clear from the page timer", async () => {
    await reachSeedStep();
    await clickCopy();
    expect(clearCalls()).toBe(0);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(REPORTED_CLEAR_AFTER_MS);
    });
    expect(clearCalls()).toBe(0);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("asks Rust to clear when the page is left after copying", async () => {
    const view = await reachSeedStep();
    await clickCopy();
    await act(async () => {
      view.unmount();
    });
    expect(clearCalls()).toBe(1);
  });

  it("asks Rust to clear when the page is left without copying", async () => {
    const view = await reachSeedStep();
    await act(async () => {
      view.unmount();
    });
    expect(clearCalls()).toBe(1);
  });

  it("tells the reader to write the phrase down when copy fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "copy_to_clipboard") return Promise.reject(new Error("backend unavailable"));
      return Promise.resolve(false);
    });
    await reachSeedStep();
    fireEvent.click(screen.getByText("Copy to clipboard"));
    expect(await screen.findByText(/write it down from the screen/i)).toBeInTheDocument();
    expect(screen.queryByText(/backend unavailable/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});

/**
 * Create on testnet and walk to the done step. The confirm step asks for four
 * randomly chosen words; read which ones from the rendered inputs.
 */
async function reachDoneOnTestnet() {
  createWallet.mockResolvedValue({
    wallet: { name: "My_Wallet", capability: "FULL", network: "TESTNET" },
    mnemonic: SEED,
  });
  render(
    <MemoryRouter>
      <CreateWallet />
    </MemoryRouter>,
  );
  fireEvent.change(screen.getByPlaceholderText("At least 8 characters"), {
    target: { value: "correct horse battery" },
  });
  fireEvent.change(screen.getByPlaceholderText("Re-enter your password"), {
    target: { value: "correct horse battery" },
  });
  fireEvent.click(screen.getByRole("button", { name: /continue/i }));
  expect(await screen.findByText(/24-word recovery phrase/i)).toBeInTheDocument();
  const words = SEED.split(" ");
  for (const word of words) expect(screen.getByText(word)).toBeInTheDocument();
  expect(screen.queryByText(/hex/i)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /saved my seed phrase/i }));

  const verify = screen.getByRole("button", { name: /verify/i });
  const inputs = screen.getAllByPlaceholderText(/enter word #/i);
  expect(inputs).toHaveLength(4);
  for (const input of inputs) {
    const n = Number(/#(\d+)/.exec(input.getAttribute("placeholder") ?? "")?.[1]);
    expect(verify).toBeDisabled();
    fireEvent.change(input, { target: { value: words[n - 1].toUpperCase() } });
  }
  expect(verify).toBeEnabled();
  fireEvent.click(verify);
  expect(await screen.findByRole("heading", { name: /wallet is ready/i })).toBeInTheDocument();
}

describe("CreateWallet testnet backup and address", () => {
  it("shows the 24 words on testnet and confirms them as words", async () => {
    await reachDoneOnTestnet();
    expect(screen.getByText("shekyl1test")).toBeInTheDocument();
  });

  it("says the address could not be read, and a retry can still show it", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_primary_address") return Promise.reject(new Error("backend unavailable"));
      if (cmd === "clear_clipboard") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
    await reachDoneOnTestnet();
    expect(screen.getByText(/copy the address from Receive/i)).toBeInTheDocument();
    expect(screen.queryByText("shekyl1test")).not.toBeInTheDocument();
    expect(screen.queryByText("Reading your address…")).not.toBeInTheDocument();

    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_primary_address") return Promise.resolve({ address: "shekyl1test" });
      if (cmd === "clear_clipboard") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByRole("button", { name: /try again/i }));
    expect(await screen.findByText("shekyl1test")).toBeInTheDocument();
  });
});
