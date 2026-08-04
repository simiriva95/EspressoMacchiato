import { useEffect, useRef, useState } from "react";
import { IdleMonitor } from "./features/idle-monitor/IdleMonitor";
import { toDurationSecs, type DurationChoice } from "./lib/duration";
import {
  ipc,
  type ActivityStrategy,
  type ScheduleWindow,
  type Settings,
  type StatusSnapshot,
} from "./lib/ipc";

const STRATEGY_LABELS: Record<ActivityStrategy, string> = {
  zero_mouse_move: "Zero-delta mouse move (invisible)",
  harmless_key_tap: "Harmless key tap (F15)",
  nudge_and_return: "Nudge 1px and return (visible)",
};

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function App() {
  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [duration, setDuration] = useState<DurationChoice>({
    kind: "indefinite",
  });
  const [untilTime, setUntilTime] = useState("17:00");
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveTimer = useRef<number | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.getStatus().then(setStatus);
    ipc.getSettings().then(setSettings);
    ipc
      .onEngineEvent((e) => {
        if (e.type === "state_changed") {
          setStatus(e.status);
        } else {
          ipc.getStatus().then(setStatus);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  // Debounced save: edits update local state immediately, persist 400ms later.
  const save = (next: Settings) => {
    setSettings(next);
    if (saveTimer.current !== null) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      ipc
        .updateSettings(next)
        .then((sanitized) => {
          setSettings(sanitized);
          setSaveError(null);
        })
        .catch((e) => setSaveError(String(e)));
    }, 400);
  };

  const on = status !== null && status.state !== "off";

  const activate = () => {
    const choice: DurationChoice =
      duration.kind === "until" ? { kind: "until", time: untilTime } : duration;
    ipc.setActive(true, toDurationSecs(choice));
  };

  return (
    <main className="mx-auto max-w-md space-y-4 p-4">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-bold">EspressoMacchiato</h1>
          <p className="text-sm text-gray-500" data-testid="state">
            {status?.state ?? "…"}
            {status?.state === "suspended" && ` — ${status.state_detail}`}
            {status?.state === "degraded" && ` — ${status.state_detail}`}
            {on &&
              status?.remaining_secs != null &&
              ` — ${Math.ceil(status.remaining_secs / 60)}m left`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => (on ? ipc.setActive(false) : activate())}
          className={`rounded px-4 py-2 font-semibold text-white ${
            on ? "bg-red-800" : "bg-green-700"
          }`}
        >
          {on ? "Deactivate" : "Activate"}
        </button>
      </header>

      {!on && (
        <div className="flex items-center gap-2 text-sm">
          <label htmlFor="duration">Duration</label>
          <select
            id="duration"
            className="rounded border border-gray-300 p-1"
            value={
              duration.kind === "minutes"
                ? String(duration.minutes)
                : duration.kind
            }
            onChange={(e) => {
              const v = e.target.value;
              if (v === "indefinite") setDuration({ kind: "indefinite" });
              else if (v === "until") setDuration({ kind: "until", time: untilTime });
              else if (v === "end_of_day") {
                setUntilTime(settings?.end_of_day ?? "18:00");
                setDuration({ kind: "until", time: settings?.end_of_day ?? "18:00" });
              } else setDuration({ kind: "minutes", minutes: Number(v) });
            }}
          >
            <option value="indefinite">Indefinite</option>
            <option value="15">15 minutes</option>
            <option value="30">30 minutes</option>
            <option value="60">1 hour</option>
            <option value="120">2 hours</option>
            <option value="240">4 hours</option>
            <option value="until">Until time…</option>
            <option value="end_of_day">
              Until end of day ({settings?.end_of_day ?? "18:00"})
            </option>
          </select>
          {duration.kind === "until" && (
            <input
              type="time"
              value={untilTime}
              onChange={(e) => setUntilTime(e.target.value)}
              className="rounded border border-gray-300 p-1"
            />
          )}
        </div>
      )}

      {status && status.degradations.length > 0 && (
        <div className="rounded border border-amber-500 bg-amber-50 p-3 text-sm">
          <p className="font-semibold">Reduced functionality</p>
          <ul className="list-disc pl-4">
            {status.degradations.map((d, i) => (
              <li key={i}>
                {d.detail}
                {d.help && <div className="mono text-xs">{d.help}</div>}
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="mt-2 rounded border border-amber-600 px-2 py-1"
            onClick={() => ipc.openPermissionSettings().catch(() => {})}
          >
            Open system settings
          </button>
        </div>
      )}

      <IdleMonitor status={status} />

      {settings && (
        <SettingsPanel
          settings={settings}
          status={status}
          onChange={save}
          saveError={saveError}
        />
      )}
    </main>
  );
}

function SettingsPanel({
  settings,
  status,
  onChange,
  saveError,
}: {
  settings: Settings;
  status: StatusSnapshot | null;
  onChange: (s: Settings) => void;
  saveError: string | null;
}) {
  const set = (patch: Partial<Settings>) => onChange({ ...settings, ...patch });
  const setConditions = (patch: Partial<Settings["conditions"]>) =>
    set({ conditions: { ...settings.conditions, ...patch } });

  return (
    <section className="space-y-4 rounded-lg border border-gray-300 p-4">
      <h2 className="text-sm font-semibold uppercase tracking-wide">
        Settings
      </h2>
      {saveError && (
        <p className="rounded border border-red-700 bg-red-50 p-2 text-xs text-red-800" role="alert">
          {saveError}
        </p>
      )}

      <label className="block text-sm">
        Poke interval: <span className="mono">{settings.interval_secs}s</span>
        <input
          type="range"
          min={10}
          max={240}
          step={5}
          value={settings.interval_secs}
          onChange={(e) => set({ interval_secs: Number(e.target.value) })}
          className="w-full"
        />
        <span className="text-xs text-gray-500">
          Presence clients typically go Away after ~5 minutes of idle; keep
          this well below that.
        </span>
      </label>

      <label className="block text-sm">
        Strategy
        <select
          value={settings.strategy}
          onChange={(e) => set({ strategy: e.target.value as ActivityStrategy })}
          className="mt-1 w-full rounded border border-gray-300 p-1"
        >
          {(status?.available_strategies ?? [settings.strategy]).map((s) => (
            <option key={s} value={s}>
              {STRATEGY_LABELS[s]}
            </option>
          ))}
        </select>
      </label>

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">Conditions</legend>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.pause_when_input_recent}
            onChange={(e) =>
              setConditions({ pause_when_input_recent: e.target.checked })
            }
          />
          Skip pokes while I'm actually using the machine
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.only_on_ac}
            onChange={(e) => setConditions({ only_on_ac: e.target.checked })}
          />
          Suspend on battery power
        </label>
        {settings.conditions.only_on_ac && (
          <label className="ml-6 flex items-center gap-2 text-sm">
            only below
            <input
              type="number"
              min={1}
              max={100}
              placeholder="any"
              value={settings.conditions.min_battery_percent ?? ""}
              onChange={(e) =>
                setConditions({
                  min_battery_percent:
                    e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="w-16 rounded border border-gray-300 p-1"
            />
            % (empty = always)
          </label>
        )}
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.only_when_process_running}
            onChange={(e) =>
              setConditions({ only_when_process_running: e.target.checked })
            }
          />
          Only while one of these apps is running
        </label>
        {settings.conditions.only_when_process_running && (
          <input
            type="text"
            aria-label="Process names, comma separated"
            value={settings.conditions.process_names.join(", ")}
            onChange={(e) =>
              setConditions({
                process_names: e.target.value.split(",").map((s) => s.trim()),
              })
            }
            className="ml-6 w-full rounded border border-gray-300 p-1 text-sm"
          />
        )}
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.pause_when_screen_locked}
            onChange={(e) =>
              setConditions({ pause_when_screen_locked: e.target.checked })
            }
          />
          Suspend while the screen is locked
        </label>
      </fieldset>

      <ScheduleEditor
        schedule={settings.schedule}
        onChange={(schedule) => set({ schedule })}
      />

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">Startup & shortcuts</legend>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.autostart}
            onChange={(e) => set({ autostart: e.target.checked })}
          />
          Start at login
        </label>
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.activate_on_start}
            onChange={(e) => set({ activate_on_start: e.target.checked })}
          />
          Activate automatically on start
        </label>
        <label className="block text-sm">
          Global hotkey
          <input
            type="text"
            value={settings.hotkey}
            onChange={(e) => set({ hotkey: e.target.value })}
            className="mt-1 w-full rounded border border-gray-300 p-1"
            placeholder="CmdOrCtrl+Alt+E (empty = none)"
          />
        </label>
        <label className="block text-sm">
          End of day (for "until end of day")
          <input
            type="time"
            value={settings.end_of_day}
            onChange={(e) => set({ end_of_day: e.target.value })}
            className="mt-1 rounded border border-gray-300 p-1"
          />
        </label>
      </fieldset>
    </section>
  );
}

function ScheduleEditor({
  schedule,
  onChange,
}: {
  schedule: Settings["schedule"];
  onChange: (s: Settings["schedule"]) => void;
}) {
  const setWindow = (i: number, patch: Partial<ScheduleWindow>) => {
    const windows = schedule.windows.map((w, j) =>
      j === i ? { ...w, ...patch } : w,
    );
    onChange({ ...schedule, windows });
  };

  return (
    <fieldset className="space-y-2">
      <legend className="text-sm font-semibold">Schedule</legend>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={schedule.enabled}
          onChange={(e) => onChange({ ...schedule, enabled: e.target.checked })}
        />
        Activate automatically during these windows
      </label>
      {schedule.enabled &&
        schedule.windows.map((w, i) => (
          <div key={i} className="ml-6 space-y-1 rounded border border-gray-200 p-2">
            <div className="flex flex-wrap gap-1">
              {DAY_LABELS.map((label, day) => (
                <button
                  key={day}
                  type="button"
                  aria-pressed={w.days.includes(day)}
                  onClick={() =>
                    setWindow(i, {
                      days: w.days.includes(day)
                        ? w.days.filter((d) => d !== day)
                        : [...w.days, day].sort(),
                    })
                  }
                  className={`rounded px-2 py-0.5 text-xs ${
                    w.days.includes(day)
                      ? "bg-green-700 text-white"
                      : "border border-gray-300"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2 text-sm">
              <input
                type="time"
                value={w.start}
                aria-label="Window start"
                onChange={(e) => setWindow(i, { start: e.target.value })}
                className="rounded border border-gray-300 p-1"
              />
              →
              <input
                type="time"
                value={w.end}
                aria-label="Window end"
                onChange={(e) => setWindow(i, { end: e.target.value })}
                className="rounded border border-gray-300 p-1"
              />
              <button
                type="button"
                onClick={() =>
                  onChange({
                    ...schedule,
                    windows: schedule.windows.filter((_, j) => j !== i),
                  })
                }
                className="ml-auto text-xs text-red-800"
              >
                Remove
              </button>
            </div>
          </div>
        ))}
      {schedule.enabled && (
        <button
          type="button"
          onClick={() =>
            onChange({
              ...schedule,
              windows: [
                ...schedule.windows,
                { days: [0, 1, 2, 3, 4], start: "09:00", end: "18:00" },
              ],
            })
          }
          className="ml-6 rounded border border-gray-400 px-2 py-1 text-xs"
        >
          Add window
        </button>
      )}
    </fieldset>
  );
}

export default App;
