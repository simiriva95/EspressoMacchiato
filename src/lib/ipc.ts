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

export interface ScheduleWindow {
  days: number[]; // 0 = Monday … 6 = Sunday
  start: string; // "HH:MM"
  end: string; // "HH:MM", exclusive; end <= start crosses midnight
}

export interface ScheduleConfig {
  enabled: boolean;
  windows: ScheduleWindow[];
}

export interface ConditionsConfig {
  only_on_ac: boolean;
  min_battery_percent: number | null;
  only_when_process_running: boolean;
  process_names: string[];
  pause_when_screen_locked: boolean;
  pause_when_input_recent: boolean;
}

export interface Settings {
  schema_version: number;
  interval_secs: number;
  strategy: ActivityStrategy;
  schedule: ScheduleConfig;
  conditions: ConditionsConfig;
  autostart: boolean;
  activate_on_start: boolean;
  hotkey: string;
  end_of_day: string; // "HH:MM"
}

export type EngineEvent =
  | { type: "state_changed"; status: StatusSnapshot }
  | { type: "poke"; report: PokeReport; poke_count: number };

export const ipc = {
  getStatus: () => invoke<StatusSnapshot>("get_status"),
  setActive: (on: boolean, durationSecs?: number) =>
    invoke<void>("set_active", { on, durationSecs: durationSecs ?? null }),
  toggle: () => invoke<void>("toggle"),
  pokeNow: () => invoke<PokeReport>("poke_now"),
  getIdleSeconds: () => invoke<number>("get_idle_seconds"),
  getPermissionStatus: () => invoke<Degradation[]>("get_permission_status"),
  openPermissionSettings: () => invoke<void>("open_permission_settings"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) =>
    invoke<Settings>("update_settings", { settings }),
  onEngineEvent: (handler: (e: EngineEvent) => void): Promise<UnlistenFn> =>
    listen<EngineEvent>("engine://event", (event) => handler(event.payload)),
};
