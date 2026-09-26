import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import CreateWallet, { SEED_CLIPBOARD_TTL_MS } from "../CreateWallet";

const SEED = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
const createWallet = vi.fn();
const setPhase = vi.fn();

vi.mock("../../context/useWallet", () => ({
  useWallet: () => ({ createWallet, setPhase }),
}));
// The wallet-dir panel polls the backend; it is not what these tests are about.
vi.mock("../../components/WalletDirAdvanced", () => ({ default: () => null }));

const writeText = vi.fn();

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(undefined);
  createWallet.mockReset();
  createWallet.mockResolvedValue({
    name: "My Wallet",
    address: "shekyl1test",
    seed: SEED,
    seed_language: "English",
    network: "mainnet",
  });
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
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
  // Let the awaited `writeText` resolve so the TTL timer gets scheduled.
  await act(async () => {
    await Promise.resolve();
  });
  expect(writeText).toHaveBeenCalledWith(SEED);
  expect(screen.getByRole("status")).toHaveTextContent(/cleared/i);
}

const clearCalls = () =>
  vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "clear_clipboard").length;

describe("CreateWallet seed clipboard mitigation", () => {
  it("clears the clipboard Rust-side once the TTL elapses", async () => {
    await reachSeedStep();
    await clickCopy();
    expect(clearCalls()).toBe(0);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(SEED_CLIPBOARD_TTL_MS - 1);
    });
    expect(clearCalls()).toBe(0);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(clearCalls()).toBe(1);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("clears the clipboard when the page is left before the TTL", async () => {
    const view = await reachSeedStep();
    await clickCopy();
    await act(async () => {
      view.unmount();
    });
    expect(clearCalls()).toBe(1);
  });

  it("never wipes a clipboard the seed was not put on", async () => {
    const view = await reachSeedStep();
    await act(async () => {
      view.unmount();
    });
    expect(clearCalls()).toBe(0);
  });
});
