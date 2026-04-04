import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Receive from "../Receive";

const MOCK_ADDRESS = "SKL1mock_account0_subaddr0...placeholder";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_address") return MOCK_ADDRESS;
    throw new Error(`unexpected: ${cmd}`);
  });
});

describe("Receive page", () => {
  it("renders the page title", () => {
    render(<Receive />);
    expect(screen.getByText("Receive SKL")).toBeInTheDocument();
  });

  it("displays the receiving address from the backend", async () => {
    render(<Receive />);

    await waitFor(() => {
      expect(screen.getByText(MOCK_ADDRESS)).toBeInTheDocument();
    });
  });

  it("copies the address to the clipboard on button click", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      writable: true,
      configurable: true,
    });

    render(<Receive />);

    await waitFor(() => {
      expect(screen.getByText(MOCK_ADDRESS)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle("Copy full address"));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(MOCK_ADDRESS);
    });
  });

  it("shows 'No wallet open' when address is empty", () => {
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));

    render(<Receive />);
    expect(screen.getByText("No wallet open")).toBeInTheDocument();
  });
});
