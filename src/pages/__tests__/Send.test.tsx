import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Send from "../Send";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("Send page", () => {
  it("renders the send form with address and amount fields", () => {
    render(<Send />);

    expect(screen.getByText("Send SKL")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("SKL1...")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("0.0000")).toBeInTheDocument();
  });

  it("displays backend error when transfer fails", async () => {
    vi.mocked(invoke).mockRejectedValue("Wallet not connected — stub implementation");
    const user = userEvent.setup();

    render(<Send />);

    await user.type(screen.getByPlaceholderText("SKL1..."), "SKL1abc123");
    await user.type(screen.getByPlaceholderText("0.0000"), "1.5");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(screen.getByText(/Wallet not connected/)).toBeInTheDocument();
    });
  });

  it("shows 'Sending...' while the transfer is in progress", async () => {
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
    const user = userEvent.setup();

    render(<Send />);

    await user.type(screen.getByPlaceholderText("SKL1..."), "SKL1abc123");
    await user.type(screen.getByPlaceholderText("0.0000"), "1.5");
    await user.click(screen.getByRole("button", { name: /send/i }));

    await waitFor(() => {
      expect(screen.getByText("Sending...")).toBeInTheDocument();
    });
  });
});
