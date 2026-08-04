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
  theme: string; // "system" | "light" | "dark"
  language: string; // "system" | "it" | "en"
  onboarding_done: boolean;
}

export type EngineEvent =
  | { type: "state_changed"; status: StatusSnapshot }
  | { type: "poke"; report: PokeReport; poke_count: number };

// Browser dev preview (vite without Tauri): serve demo data so the UI can
// be developed and screenshotted outside the app shell. Never active inside
// Tauri, where __TAURI_INTERNALS__ exists.
const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const demoStatus: StatusSnapshot = {
  state: "active",
  state_detail: "Manual",
  interval_secs: 60,
  strategy: "zero_mouse_move",
  available_strategies: ["zero_mouse_move", "harmless_key_tap", "nudge_and_return"],
  pause_when_input_recent: true,
  inhibitor_active: true,
  poke_count: 12,
  last_poke_unix_ms: Date.now() - 21_000,
  last_poke_ok: true,
  last_poke_error: null,
  next_poke_in_secs: 39,
  remaining_secs: null,
  idle_source: "CGEventSource",
  degradations: [],
};

const demoSettings: Settings = {
  schema_version: 1,
  interval_secs: 60,
  strategy: "zero_mouse_move",
  schedule: { enabled: false, windows: [] },
  conditions: {
    only_on_ac: false,
    min_battery_percent: null,
    only_when_process_running: false,
    process_names: ["Teams", "teams", "msedge", "chrome"],
    pause_when_screen_locked: false,
    pause_when_input_recent: true,
  },
  autostart: false,
  activate_on_start: false,
  hotkey: "CmdOrCtrl+Alt+E",
  end_of_day: "18:00",
  theme: "system",
  language: "system",
  onboarding_done: true,
};

const browserDemo: typeof tauriIpc = {
  getStatus: () => Promise.resolve(demoStatus),
  setActive: () => Promise.resolve(),
  toggle: () => Promise.resolve(),
  pokeNow: () =>
    Promise.resolve({
      ok: true,
      skipped: false,
      error: null,
      idle_before: 21.4,
      idle_after: 0.1,
    }),
  getIdleSeconds: () => Promise.resolve((Date.now() / 1000) % 60),
  getPermissionStatus: () => Promise.resolve([]),
  openPermissionSettings: () => Promise.resolve(),
  openSettingsWindow: () => Promise.resolve(),
  getSettings: () => Promise.resolve(demoSettings),
  updateSettings: (s) => Promise.resolve(s),
  onEngineEvent: () => Promise.resolve(() => {}),
};

const tauriIpc = {
  getStatus: () => invoke<StatusSnapshot>("get_status"),
  setActive: (on: boolean, durationSecs?: number) =>
    invoke<void>("set_active", { on, durationSecs: durationSecs ?? null }),
  toggle: () => invoke<void>("toggle"),
  pokeNow: () => invoke<PokeReport>("poke_now"),
  getIdleSeconds: () => invoke<number>("get_idle_seconds"),
  getPermissionStatus: () => invoke<Degradation[]>("get_permission_status"),
  openPermissionSettings: () => invoke<void>("open_permission_settings"),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) =>
    invoke<Settings>("update_settings", { settings }),
  onEngineEvent: (handler: (e: EngineEvent) => void): Promise<UnlistenFn> =>
    listen<EngineEvent>("engine://event", (event) => handler(event.payload)),
};

export const ipc = isTauri ? tauriIpc : browserDemo;
