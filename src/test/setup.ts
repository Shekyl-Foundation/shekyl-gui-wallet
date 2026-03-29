import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// Stub the Tauri invoke API globally so tests that don't explicitly mock it
// won't throw "window.__TAURI_INTERNALS__ is not defined".
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("invoke not mocked")),
}));
