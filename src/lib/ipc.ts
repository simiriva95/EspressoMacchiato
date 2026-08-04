// Thin wrapper around Tauri IPC so components stay testable without Tauri.
// Tests replace this module with vi.mock().

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ActivityStrategy =
  | "zero_mouse_move"
  | "harmless_key_tap"
  | "nudge_and_return";

export interface Degradation {
  what: "poke_unavailable" | "inhibit_unavailable" | "idle_unreadable";
  detail: string;
  help: string | null;
}

export interface StatusSnapshot {
  state: "off" | "active" | "suspended" | "degraded";
  state_detail: string;
  interval_secs: number;
  strategy: ActivityStrategy;
  available_strategies: ActivityStrategy[];
  pause_when_input_recent: boolean;
  inhibitor_active: boolean;
  poke_count: number;
  last_poke_unix_ms: number | null;
  last_poke_ok: boolean | null;
  last_poke_error: string | null;
  next_poke_in_secs: number | null;
  remaining_secs: number | null;
  idle_source: string;
  degradations: Degradation[];
}

export interface PokeReport {
  ok: boolean;
  skipped: boolean;
  error: string | null;
  idle_before: number | null;
  idle_after: number | null;
}

export type EngineEvent =
  | { type: "state_changed"; status: StatusSnapshot }
  | { type: "poke"; report: PokeReport; poke_count: number };

export const ipc = {
  getStatus: () => invoke<StatusSnapshot>("get_status"),
  setActive: (on: boolean) => invoke<void>("set_active", { on }),
  toggle: () => invoke<void>("toggle"),
  setIntervalSecs: (secs: number) =>
    invoke<number>("set_interval_secs", { secs }),
  setPauseWhenInputRecent: (on: boolean) =>
    invoke<void>("set_pause_when_input_recent", { on }),
  setStrategy: (strategy: ActivityStrategy) =>
    invoke<void>("set_strategy", { strategy }),
  pokeNow: () => invoke<PokeReport>("poke_now"),
  getIdleSeconds: () => invoke<number>("get_idle_seconds"),
  getPermissionStatus: () => invoke<Degradation[]>("get_permission_status"),
  openPermissionSettings: () => invoke<void>("open_permission_settings"),
  onEngineEvent: (handler: (e: EngineEvent) => void): Promise<UnlistenFn> =>
    listen<EngineEvent>("engine://event", (event) => handler(event.payload)),
};
