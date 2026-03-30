import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import EmissionGauge from "../EmissionGauge";

describe("EmissionGauge", () => {
  it("renders the label and display value", () => {
    render(<EmissionGauge value={50} label="Stake Ratio" display="50.00%" />);

    expect(screen.getByText("50.00%")).toBeInTheDocument();
    expect(screen.getByText("Stake Ratio")).toBeInTheDocument();
  });

  it("renders an SVG circle gauge", () => {
    const { container } = render(
      <EmissionGauge value={75} label="Test" display="75%" />,
    );

    expect(container.querySelector("svg")).toBeTruthy();
    expect(container.querySelectorAll("circle").length).toBe(2);
  });

  it("clamps the percentage to 100", () => {
    const { container } = render(
      <EmissionGauge value={200} max={100} label="Over" display="200%" />,
    );

    const arcs = container.querySelectorAll("circle");
    const dashOffset = arcs[1].getAttribute("stroke-dashoffset");
    expect(Number(dashOffset)).toBe(0);
  });
});
