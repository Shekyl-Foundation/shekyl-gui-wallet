import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DaemonProvider } from "../../context/DaemonContext";
import Help from "../Help";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

function renderHelp() {
  return render(
    <MemoryRouter>
      <DaemonProvider>
        <Help />
      </DaemonProvider>
    </MemoryRouter>,
  );
}

describe("Help", () => {
  it("renders the Help Center heading", () => {
    renderHelp();
    expect(screen.getByText("Help Center")).toBeInTheDocument();
  });

  it("renders all section titles", () => {
    renderHelp();
    expect(screen.getByText("Getting Started")).toBeInTheDocument();
    expect(screen.getByText("Mining Guide")).toBeInTheDocument();
    expect(screen.getByText("Staking Guide")).toBeInTheDocument();
    expect(screen.getByText("Post-Quantum Cryptography")).toBeInTheDocument();
    expect(screen.getByText("Glossary")).toBeInTheDocument();
  });

  it("opens Getting Started by default", () => {
    renderHelp();
    expect(screen.getByText("What is Shekyl?")).toBeInTheDocument();
    expect(screen.getByText("Connecting to the Daemon")).toBeInTheDocument();
  });

  it("toggles sections on click", () => {
    renderHelp();
    expect(screen.getByText("What is Shekyl?")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Mining Guide"));
    expect(screen.getByText("What is Mining?")).toBeInTheDocument();
    expect(screen.queryByText("What is Shekyl?")).not.toBeInTheDocument();
  });

  it("collapses a section when clicking it again", () => {
    renderHelp();
    expect(screen.getByText("What is Shekyl?")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Getting Started"));
    expect(screen.queryByText("What is Shekyl?")).not.toBeInTheDocument();
  });

  it("shows PQC content when expanded", () => {
    renderHelp();
    fireEvent.click(screen.getByText("Post-Quantum Cryptography"));
    expect(screen.getByText("Why PQC Matters")).toBeInTheDocument();
    expect(screen.getAllByText(/ML-DSA-65/).length).toBeGreaterThan(0);
    expect(screen.getByText("V4 Roadmap")).toBeInTheDocument();
  });

  it("shows glossary terms when expanded", () => {
    renderHelp();
    fireEvent.click(screen.getByText("Glossary"));
    expect(screen.getByText("Atomic Unit")).toBeInTheDocument();
    expect(screen.getByText("Hash Rate")).toBeInTheDocument();
    expect(screen.getByText("Ring Signature")).toBeInTheDocument();
  });

  it("shows staking guide content when expanded", () => {
    renderHelp();
    fireEvent.click(screen.getByText("Staking Guide"));
    expect(screen.getByText(/claim-based staking model/)).toBeInTheDocument();
    expect(screen.getByText("Privacy Benefit")).toBeInTheDocument();
  });
});
