import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import NetworkBadge from "../NetworkBadge";

describe("NetworkBadge", () => {
  it("renders the network name as uppercase text", () => {
    render(<NetworkBadge network="mainnet" />);
    expect(screen.getByText("mainnet")).toBeInTheDocument();
  });

  it("applies emerald styling for mainnet", () => {
    const { container } = render(<NetworkBadge network="mainnet" />);
    const badge = container.querySelector("span")!;
    expect(badge.className).toContain("text-emerald-300");
  });

  it("applies amber styling for testnet", () => {
    const { container } = render(<NetworkBadge network="testnet" />);
    const badge = container.querySelector("span")!;
    expect(badge.className).toContain("text-amber-300");
  });

  it("applies blue styling for stagenet", () => {
    const { container } = render(<NetworkBadge network="stagenet" />);
    const badge = container.querySelector("span")!;
    expect(badge.className).toContain("text-blue-300");
  });

  it("falls back to mainnet styling for unknown networks", () => {
    const { container } = render(<NetworkBadge network="devnet" />);
    const badge = container.querySelector("span")!;
    expect(badge.className).toContain("text-emerald-300");
  });
});
