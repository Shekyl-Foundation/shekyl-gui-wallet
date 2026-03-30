import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect } from "vitest";
import Sidebar from "../Sidebar";

function renderSidebar(initialRoute = "/") {
  return render(
    <MemoryRouter initialEntries={[initialRoute]}>
      <Sidebar />
    </MemoryRouter>,
  );
}

describe("Sidebar", () => {
  it("renders all navigation links", () => {
    renderSidebar();

    expect(screen.getByText("Dashboard")).toBeInTheDocument();
    expect(screen.getByText("Send")).toBeInTheDocument();
    expect(screen.getByText("Receive")).toBeInTheDocument();
    expect(screen.getByText("Mining")).toBeInTheDocument();
    expect(screen.getByText("Staking")).toBeInTheDocument();
    expect(screen.getByText("Transactions")).toBeInTheDocument();
    expect(screen.getByText("Chain Health")).toBeInTheDocument();
    expect(screen.getByText("Help")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("renders the Shekyl branding", () => {
    renderSidebar();
    expect(screen.getByText("Shekyl")).toBeInTheDocument();
    expect(screen.getByAltText("Shekyl")).toBeInTheDocument();
  });

  it("renders the version string", () => {
    renderSidebar();
    expect(screen.getByText(/Shekyl Wallet v/)).toBeInTheDocument();
  });

  it("highlights the active route", () => {
    renderSidebar("/send");
    const sendLink = screen.getByText("Send").closest("a")!;
    expect(sendLink.className).toContain("text-gold-400");
  });
});
