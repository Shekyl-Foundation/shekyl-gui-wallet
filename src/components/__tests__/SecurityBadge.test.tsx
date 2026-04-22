import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, it, expect, vi } from "vitest";
import SecurityBadge from "../SecurityBadge";

vi.mock("../../context/useDaemon", () => ({
  useDaemon: vi.fn(),
}));

import { useDaemon } from "../../context/useDaemon";

describe("SecurityBadge", () => {
  it("renders 3-layer badge with anonymity set size when security data is available", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: null,
      pqc: {
        enabled: true,
        scheme: "Hybrid",
        classical: "Ed25519",
        post_quantum: "ML-DSA-65",
        tx_version: 3,
        description: "Protected by hybrid signatures",
      },
      security: {
        scheme: "Hybrid",
        classical: "Ed25519",
        post_quantum: "ML-DSA-65 (FIPS 204)",
        tx_version: 3,
        anonymity_set_size: 142837,
        tree_depth: 12,
        tree_root_short: "a3f7c2e1",
        reference_block_window: 100,
        proof_type: "FCMP++ Full-Chain Membership",
        max_inputs: 8,
        estimated_proof_size_kb: 4.5,
        paths_precomputed: true,
      },
      status: null,
      connected: false,
      hasEverConnected: false,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(
      <MemoryRouter>
        <SecurityBadge />
      </MemoryRouter>,
    );

    expect(screen.getByText(/3-layer/)).toBeInTheDocument();
    expect(screen.getByText(/142\.8K outputs/)).toBeInTheDocument();
  });

  it("falls back to PQC badge when no security data", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: null,
      pqc: {
        enabled: true,
        scheme: "Hybrid",
        classical: "Ed25519",
        post_quantum: "ML-DSA-65",
        tx_version: 3,
        description: "Protected by hybrid signatures",
      },
      security: null,
      status: null,
      connected: false,
      hasEverConnected: false,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(
      <MemoryRouter>
        <SecurityBadge />
      </MemoryRouter>,
    );

    expect(screen.getByText(/Ed25519/)).toBeInTheDocument();
  });

  it("renders nothing when no data is available", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: null,
      pqc: null,
      security: null,
      status: null,
      connected: false,
      hasEverConnected: false,
      waitingSeconds: 0,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    const { container } = render(
      <MemoryRouter>
        <SecurityBadge />
      </MemoryRouter>,
    );
    expect(container.innerHTML).toBe("");
  });
});
