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

export interface ProcessUsage {
  name: string;
  cpu_percent: number;
}

/** serde's default Duration encoding. */
export interface DurationSerde {
  secs: number;
  nanos: number;
}

export interface PowerSnapshot {
  on_ac: boolean;
  percent: number | null;
  cycle_count: number | null;
  design_capacity_mah: number | null;
  max_capacity_mah: number | null;
  health_percent: number | null;
  temperature_c: number | null;
  voltage_v: number | null;
  amperage_ma: number | null;
  watts: number | null;
  time_to_empty: DurationSerde | null;
  time_to_full: DurationSerde | null;
  cpu_percent: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  uptime_secs: number | null;
  top_energy_processes: ProcessUsage[];
}

export interface PowerSample {
  unix_ms: number;
  percent: number | null;
  watts: number | null;
  temperature_c: number | null;
  cpu_percent: number;
}

export interface AlertsConfig {
  charge_reminder: boolean;
  charge_target_percent: number;
  low_battery: boolean;
  low_battery_percent: number;
  overheat: boolean;
  overheat_celsius: number;
  timer_expired: boolean;
  calibration_reminder: boolean;
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
  auto_activate_on_call: boolean;
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
  alerts: AlertsConfig;
  menu_bar_metrics: string[]; // subset of countdown|battery|watts, max 2
  menu_bar_ring: string; // "off" | "timer" | "battery"
  hud_enabled: boolean;
  accent: string; // hex
  sound_on_activate: boolean;
}

export interface StatsReport {
  week_active_secs: number;
  week_pokes: number;
  shots_today: number;
  health_now: number | null;
  health_month_ago: number | null;
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
    auto_activate_on_call: false,
  },
  autostart: false,
  activate_on_start: false,
  hotkey: "CmdOrCtrl+Alt+E",
  end_of_day: "18:00",
  theme: "system",
  language: "system",
  onboarding_done: true,
  alerts: {
    charge_reminder: false,
    charge_target_percent: 80,
    low_battery: false,
    low_battery_percent: 15,
    overheat: false,
    overheat_celsius: 45,
    timer_expired: false,
    calibration_reminder: false,
  },
  menu_bar_metrics: [],
  menu_bar_ring: "timer",
  hud_enabled: false,
  accent: "#e8a54c",
  sound_on_activate: false,
};

const demoPower: PowerSnapshot = {
  on_ac: false,
  percent: 73,
  cycle_count: 125,
  design_capacity_mah: 6249,
  max_capacity_mah: 5913,
  health_percent: 94.6,
  temperature_c: 30.7,
  voltage_v: 10.8,
  amperage_ma: -1450,
  watts: 15.7,
  time_to_empty: { secs: 3 * 3600, nanos: 0 },
  time_to_full: null,
  cpu_percent: 12.4,
  memory_used_bytes: 12_884_901_888,
  memory_total_bytes: 25_769_803_776,
  uptime_secs: 86_400 * 2 + 3600 * 3,
  top_energy_processes: [
    { name: "chrome", cpu_percent: 34.1 },
    { name: "MSTeams", cpu_percent: 12.9 },
    { name: "node", cpu_percent: 4.2 },
  ],
};

const demoHistory: PowerSample[] = Array.from({ length: 240 }, (_, i) => ({
  unix_ms: Date.now() - (240 - i) * 10_000,
  percent: 90 - i * 0.07,
  watts: 12 + 6 * Math.abs(Math.sin(i / 12)),
  temperature_c: 29 + 3 * Math.abs(Math.sin(i / 40)),
  cpu_percent: 8 + 20 * Math.abs(Math.sin(i / 7)),
}));

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
  requestPermission: () => Promise.resolve(true),
  openPermissionSettings: () => Promise.resolve(),
  openSettingsWindow: () => Promise.resolve(),
  getSettings: () => Promise.resolve(demoSettings),
  updateSettings: (s) => Promise.resolve(s),
  onEngineEvent: () => Promise.resolve(() => {}),
  getNextMeetingEnd: () =>
    Promise.resolve(Math.round(Date.now() / 1000) + 45 * 60),
  getStats: () =>
    Promise.resolve({
      week_active_secs: 14 * 3600 + 1200,
      week_pokes: 812,
      shots_today: 3,
      health_now: 94.6,
      health_month_ago: 95.1,
    }),
  getPower: () => Promise.resolve(demoPower),
  getPowerHistory: () => Promise.resolve(demoHistory),
  onPowerSample: () => Promise.resolve(() => {}),
};

const tauriIpc = {
  getStatus: () => invoke<StatusSnapshot>("get_status"),
  setActive: (on: boolean, durationSecs?: number) =>
    invoke<void>("set_active", { on, durationSecs: durationSecs ?? null }),
  toggle: () => invoke<void>("toggle"),
  pokeNow: () => invoke<PokeReport>("poke_now"),
  getIdleSeconds: () => invoke<number>("get_idle_seconds"),
  getPermissionStatus: () => invoke<Degradation[]>("get_permission_status"),
  requestPermission: () => invoke<boolean>("request_permission"),
  openPermissionSettings: () => invoke<void>("open_permission_settings"),
  openSettingsWindow: () => invoke<void>("open_settings_window"),
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) =>
    invoke<Settings>("update_settings", { settings }),
  onEngineEvent: (handler: (e: EngineEvent) => void): Promise<UnlistenFn> =>
    listen<EngineEvent>("engine://event", (event) => handler(event.payload)),
  /** Unix seconds when the current/imminent meeting ends (macOS EventKit).
   * Rejects when the calendar permission is denied or unavailable. */
  getNextMeetingEnd: () => invoke<number | null>("get_next_meeting_end"),
  getStats: () => invoke<StatsReport>("get_stats"),
  getPower: () => invoke<PowerSnapshot | null>("get_power"),
  getPowerHistory: () => invoke<PowerSample[]>("get_power_history"),
  onPowerSample: (handler: (s: PowerSample) => void): Promise<UnlistenFn> =>
    listen<PowerSample>("power://sample", (event) => handler(event.payload)),
};

export const ipc = isTauri ? tauriIpc : browserDemo;
