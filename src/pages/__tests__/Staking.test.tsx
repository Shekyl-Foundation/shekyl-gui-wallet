import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Staking from "../Staking";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_chain_health" || cmd === "get_wallet_status") {
      return null;
    }
    if (cmd === "list_shard_preview_fixtures") {
      return [];
    }
    return null;
  });
});

function renderStaking() {
  return render(
    <DaemonProvider>
      <Staking />
    </DaemonProvider>,
  );
}

describe("Staking (honesty mode)", () => {
  it("renders the honesty banner and archival model", () => {
    renderStaking();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(
      screen.getByText(/Archival staking is not available in this build/),
    ).toBeInTheDocument();
    expect(screen.getByText("What staking will mean")).toBeInTheDocument();
    expect(screen.getByText("For future operators")).toBeInTheDocument();
  });

  it("does not offer claim-era stake or claim actions", () => {
    renderStaking();
    expect(screen.queryByText(/Select Staking Tier/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Stake at Tier/)).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText(/Amount to stake/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Claim/i })).not.toBeInTheDocument();
  });
});
