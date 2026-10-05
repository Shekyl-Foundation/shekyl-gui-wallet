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
    encoding: "mnemonic",
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

const RAW_SEED = "ab".repeat(32);

async function reachDoneWithRawSeed() {
  createWallet.mockResolvedValue({
    wallet: { name: "My_Wallet", capability: "FULL", network: "TESTNET" },
    encoding: "raw_seed_hex",
    raw_seed_hex: RAW_SEED,
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
  expect(await screen.findByTestId("raw-seed")).toHaveTextContent(RAW_SEED);
  expect(screen.queryByText(/24-word recovery phrase/i)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /saved this hex seed/i }));
  fireEvent.change(screen.getByLabelText(/re-enter the 64-character hex seed/i), {
    target: { value: RAW_SEED.toUpperCase() },
  });
  fireEvent.click(screen.getByRole("button", { name: /verify/i }));
  expect(await screen.findByRole("heading", { name: /wallet is ready/i })).toBeInTheDocument();
}

describe("CreateWallet testnet backup and address", () => {
  it("shows the hex seed and confirms it by re-entry, not as a word", async () => {
    await reachDoneWithRawSeed();
    expect(screen.getByText("shekyl1test")).toBeInTheDocument();
    expect(screen.queryByText(/word #/i)).not.toBeInTheDocument();
  });

  it("says the address could not be read, and a retry can still show it", async () => {
    vi.mocked(invoke).mockImplementation((cmd) => {
      if (cmd === "get_primary_address") return Promise.reject(new Error("backend unavailable"));
      if (cmd === "clear_clipboard") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
    await reachDoneWithRawSeed();
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
