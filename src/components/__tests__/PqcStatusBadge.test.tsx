import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import PqcStatusBadge from "../PqcStatusBadge";

vi.mock("../../context/useDaemon", () => ({
  useDaemon: vi.fn(),
}));

import { useDaemon } from "../../context/useDaemon";

describe("PqcStatusBadge", () => {
  it("renders hybrid signature scheme info when PQC is available", () => {
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
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    render(<PqcStatusBadge />);

    expect(screen.getByText(/Ed25519/)).toBeInTheDocument();
    expect(screen.getByText(/ML-DSA-65/)).toBeInTheDocument();
  });

  it("renders nothing when PQC data is null", () => {
    vi.mocked(useDaemon).mockReturnValue({
      health: null,
      pqc: null,
      loading: false,
      error: null,
      refresh: vi.fn(),
    });

    const { container } = render(<PqcStatusBadge />);
    expect(container.innerHTML).toBe("");
  });
});
