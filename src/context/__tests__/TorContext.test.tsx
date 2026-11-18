import { act, render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { TorProvider } from "../TorContext";
import { useTor } from "../useTor";
import type { TorRead } from "../../types/tor";

function tor(text: string): TorRead {
  return { shown: true, text, sends: "held", fixes: [] };
}

/** Shows the provider's read, and a button that asks for a fresh one. */
function Probe() {
  const { read, refresh } = useTor();
  return (
    <>
      <p data-testid="read">{read ? read.text : "none"}</p>
      <button onClick={() => refresh()}>refresh</button>
    </>
  );
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("TorProvider", () => {
  /**
   * A read slower than the beat is still shown (the finding on #71's
   * Settings poll, here for the header and the send review). The next read
   * is asked for only once this one has finished. Red edit: read on a fixed
   * `setInterval` with the numbering kept (each slow answer is numbered out
   * by the read begun behind it, and nothing is ever shown).
   */
  it("shows a read slower than its beat", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd !== "get_tor_status") return null;
      await new Promise((done) => setTimeout(done, 5_600));
      return tor("slow but shown");
    });
    render(
      <TorProvider>
        <Probe />
      </TorProvider>,
    );
    expect(
      await screen.findByText("slow but shown", {}, { timeout: 9_000 }),
    ).toBeInTheDocument();
  }, 12_000);

  /**
   * An older answer never paints over a newer one: a refresh asked for
   * after a fix is taken answers first, and the poll's earlier, slower read
   * is dropped when it lands. Red edit: drop the numbering (the stale
   * answer is shown last).
   */
  it("never paints an older read over a newer one", async () => {
    let call = 0;
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd !== "get_tor_status") return null;
      call += 1;
      if (call === 1) {
        await new Promise((done) => setTimeout(done, 300));
        return tor("older");
      }
      return tor("newer");
    });
    render(
      <TorProvider>
        <Probe />
      </TorProvider>,
    );
    // The poll's first read is in flight; a refresh overtakes it.
    await act(async () => {
      await new Promise((done) => setTimeout(done, 20));
    });
    await act(async () => {
      screen.getByRole("button", { name: "refresh" }).click();
    });
    expect(await screen.findByText("newer")).toBeInTheDocument();
    await act(async () => {
      await new Promise((done) => setTimeout(done, 400));
    });
    expect(screen.getByTestId("read")).toHaveTextContent("newer");
  });
});
