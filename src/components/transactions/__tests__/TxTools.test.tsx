import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import TxTools from "../TxTools";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function Harness() {
  const [lookupId, setLookupId] = useState("");
  return <TxTools lookupId={lookupId} onLookupId={setLookupId} />;
}

describe("TxTools", () => {
  it("does not abandon a send until the person confirms", async () => {
    vi.mocked(invoke).mockResolvedValue({});
    const user = userEvent.setup();
    render(<Harness />);
    await user.type(screen.getByLabelText("Transaction id"), "ab");
    await user.click(screen.getByRole("button", { name: "Abandon send" }));
    expect(invoke).not.toHaveBeenCalled();
    expect(screen.getByText(/stops tracking this send/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Keep it" }));
    expect(invoke).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Abandon send" }));
    await user.click(screen.getByRole("button", { name: "Abandon this send" }));
    expect(await screen.findByText(/Send abandoned/)).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.map(([cmd]) => cmd)).toEqual(["abandon_tx"]);
  });

  it("shows a looked-up amount in SKL", async () => {
    vi.mocked(invoke).mockResolvedValue({
      transfer: { id: "aa", tx_hash: "aa", amount: "1000000000", state: "CONFIRMED" },
    });
    const user = userEvent.setup();
    render(<Harness />);
    await user.type(screen.getByLabelText("Look up by id"), "aa");
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByText("Confirmed: 1.000000 SKL (id aa)")).toBeInTheDocument();
  });
});
