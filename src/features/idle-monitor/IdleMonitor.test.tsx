import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StatusSnapshot } from "../../lib/ipc";

vi.mock("../../lib/ipc", () => ({
  ipc: {
    getIdleSeconds: vi.fn().mockResolvedValue(42),
    pokeNow: vi.fn().mockResolvedValue({
      ok: true,
      skipped: false,
      error: null,
      idle_before: 42,
      idle_after: 0.1,
    }),
  },
}));

import { IdleMonitor } from "./IdleMonitor";

const baseStatus: StatusSnapshot = {
  state: "active",
  state_detail: "Manual",
  interval_secs: 60,
  strategy: "zero_mouse_move",
  available_strategies: ["zero_mouse_move"],
  pause_when_input_recent: true,
  inhibitor_active: true,
  poke_count: 7,
  last_poke_unix_ms: 1_700_000_000_000,
  last_poke_ok: true,
  last_poke_error: null,
  next_poke_in_secs: 33,
  remaining_secs: null,
  idle_source: "mock",
  degradations: [],
};

describe("IdleMonitor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows idle value from the OS and its source", async () => {
    render(<IdleMonitor status={baseStatus} />);
    expect(await screen.findByTestId("idle-value")).toHaveTextContent("42s");
    expect(screen.getByTestId("idle-source")).toHaveTextContent("mock");
  });

  it("shows session poke stats", async () => {
    render(<IdleMonitor status={baseStatus} />);
    expect(screen.getByTestId("poke-count")).toHaveTextContent("7");
    expect(screen.getByTestId("next-poke")).toHaveTextContent("33s");
    expect(screen.getByTestId("last-result")).toHaveTextContent("ok");
  });

  it("renders placeholders when status is missing", async () => {
    render(<IdleMonitor status={null} />);
    expect(screen.getByTestId("poke-count")).toHaveTextContent("0");
    expect(screen.getByTestId("next-poke")).toHaveTextContent("—");
  });
});
