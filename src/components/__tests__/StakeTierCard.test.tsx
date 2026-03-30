import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import StakeTierCard from "../StakeTierCard";
import type { TierYield } from "../../types/daemon";

const tier: TierYield = {
  tier: 1,
  lock_blocks: 21600,
  lock_duration_hours: 720,
  yield_multiplier: 1.5,
  estimated_apy: 12.5,
};

describe("StakeTierCard", () => {
  it("renders tier info", () => {
    render(<StakeTierCard tier={tier} />);

    expect(screen.getByText(/Tier 1/)).toBeInTheDocument();
    expect(screen.getByText(/Medium/)).toBeInTheDocument();
    expect(screen.getByText("1.5x")).toBeInTheDocument();
    expect(screen.getByText("12.5%")).toBeInTheDocument();
    expect(screen.getByText(/Est. APY/)).toBeInTheDocument();
  });

  it("shows lock duration", () => {
    render(<StakeTierCard tier={tier} />);
    expect(screen.getByText(/1 months/)).toBeInTheDocument();
  });

  it("calls onSelect when clicked", () => {
    const onSelect = vi.fn();
    render(<StakeTierCard tier={tier} onSelect={onSelect} />);

    fireEvent.click(screen.getByRole("button"));
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("applies selected styling", () => {
    const { container } = render(<StakeTierCard tier={tier} selected />);
    const button = container.querySelector("button")!;
    expect(button.className).toContain("border-gold-500");
  });
});
