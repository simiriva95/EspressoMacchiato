// The settings window: full controls, idle monitor, degradations, and the
// first-run wizard when a permission is missing.

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DegradationsCard } from "../components/DegradationsCard";
import { StateAnnouncer } from "../components/StateAnnouncer";
import { EngineControls } from "../features/engine/EngineControls";
import { IdleMonitor } from "../features/idle-monitor/IdleMonitor";
import { Onboarding } from "../features/onboarding/Onboarding";
import { PowerPanel } from "../features/power/PowerPanel";
import {
  ipc,
  type ActivityStrategy,
  type ScheduleWindow,
  type Settings,
  type StatusSnapshot,
} from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

export function SettingsWindow() {
  const { t } = useTranslation();
  const { status, settings, save, saveError, pokeSignal } = useEngine();
  const [showOnboarding, setShowOnboarding] = useState(false);
  const onboardingChecked = useRef(false);

  // First run: open the wizard only if a permission is actually missing.
  useEffect(() => {
    if (!settings || onboardingChecked.current) return;
    onboardingChecked.current = true;
    if (!settings.onboarding_done) {
      ipc.getPermissionStatus().then((degradations) => {
        if (degradations.length > 0) setShowOnboarding(true);
        else save({ ...settings, onboarding_done: true });
      });
    }
  }, [settings, save]);

  return (
    <main className="mx-auto max-w-md space-y-4 p-4">
      <StateAnnouncer status={status} />
      <header>
        <h1 className="text-sm font-semibold uppercase tracking-wide text-ink-2">
          {t("app.name")}
        </h1>
      </header>

      <EngineControls status={status} settings={settings} />
      <DegradationsCard degradations={status?.degradations ?? []} />
      <IdleMonitor status={status} pokeSignal={pokeSignal} />
      <PowerPanel />

      {settings && (
        <SettingsForm
          settings={settings}
          status={status}
          onChange={save}
          saveError={saveError}
          onReopenOnboarding={() => setShowOnboarding(true)}
        />
      )}

      {showOnboarding && settings && (
        <Onboarding
          settings={settings}
          onSave={save}
          onClose={() => setShowOnboarding(false)}
        />
      )}
    </main>
  );
}

function SettingsForm({
  settings,
  status,
  onChange,
  saveError,
  onReopenOnboarding,
}: {
  settings: Settings;
  status: StatusSnapshot | null;
  onChange: (s: Settings) => void;
  saveError: string | null;
  onReopenOnboarding: () => void;
}) {
  const { t } = useTranslation();
  const set = (patch: Partial<Settings>) => onChange({ ...settings, ...patch });
  const setConditions = (patch: Partial<Settings["conditions"]>) =>
    set({ conditions: { ...settings.conditions, ...patch } });

  return (
    <section className="space-y-5 rounded-lg border border-line bg-surface p-4">
      <h2 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
        {t("settings.title")}
      </h2>
      {saveError && (
        <p
          className="rounded border border-alert p-2 text-xs text-alert"
          role="alert"
        >
          {t("common.saveError", { error: saveError })}
        </p>
      )}

      <label className="block text-sm">
        {t("settings.interval")}:{" "}
        <span className="mono">{settings.interval_secs}s</span>
        <input
          type="range"
          min={10}
          max={240}
          step={5}
          value={settings.interval_secs}
          onChange={(e) => set({ interval_secs: Number(e.target.value) })}
          className="w-full"
        />
        <span className="text-xs text-ink-2">{t("settings.intervalHelp")}</span>
      </label>

      <label className="block text-sm">
        {t("settings.strategy")}
        <select
          value={settings.strategy}
          onChange={(e) => set({ strategy: e.target.value as ActivityStrategy })}
          className="mt-1 min-h-8 w-full rounded border border-line bg-bg p-1"
        >
          {(status?.available_strategies ?? [settings.strategy]).map((s) => (
            <option key={s} value={s}>
              {t(`settings.strategies.${s}`)}
            </option>
          ))}
        </select>
      </label>

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">
          {t("settings.conditions")}
        </legend>
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.pause_when_input_recent}
            onChange={(e) =>
              setConditions({ pause_when_input_recent: e.target.checked })
            }
          />
          {t("settings.pauseWhenInputRecent")}
        </label>
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.only_on_ac}
            onChange={(e) => setConditions({ only_on_ac: e.target.checked })}
          />
          {t("settings.onlyOnAc")}
        </label>
        {settings.conditions.only_on_ac && (
          <label className="ml-6 flex min-h-8 items-center gap-2 text-sm">
            {t("settings.onlyBelow")}
            <input
              type="number"
              min={1}
              max={100}
              value={settings.conditions.min_battery_percent ?? ""}
              onChange={(e) =>
                setConditions({
                  min_battery_percent:
                    e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="min-h-8 w-16 rounded border border-line bg-bg p-1"
            />
            {t("settings.anyLevel")}
          </label>
        )}
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.only_when_process_running}
            onChange={(e) =>
              setConditions({ only_when_process_running: e.target.checked })
            }
          />
          {t("settings.onlyWhenProcessRunning")}
        </label>
        {settings.conditions.only_when_process_running && (
          <input
            type="text"
            aria-label={t("settings.processNames")}
            value={settings.conditions.process_names.join(", ")}
            onChange={(e) =>
              setConditions({
                process_names: e.target.value.split(",").map((s) => s.trim()),
              })
            }
            className="ml-6 min-h-8 w-full rounded border border-line bg-bg p-1 text-sm"
          />
        )}
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.conditions.pause_when_screen_locked}
            onChange={(e) =>
              setConditions({ pause_when_screen_locked: e.target.checked })
            }
          />
          {t("settings.pauseWhenScreenLocked")}
        </label>
      </fieldset>

      <ScheduleEditor
        schedule={settings.schedule}
        onChange={(schedule) => set({ schedule })}
      />

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">
          {t("settings.startup")}
        </legend>
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.autostart}
            onChange={(e) => set({ autostart: e.target.checked })}
          />
          {t("settings.autostart")}
        </label>
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.activate_on_start}
            onChange={(e) => set({ activate_on_start: e.target.checked })}
          />
          {t("settings.activateOnStart")}
        </label>
        <label className="block text-sm">
          {t("settings.hotkey")}
          <input
            type="text"
            value={settings.hotkey}
            onChange={(e) => set({ hotkey: e.target.value })}
            className="mono mt-1 min-h-8 w-full rounded border border-line bg-bg p-1"
            placeholder={t("settings.hotkeyPlaceholder")}
          />
        </label>
        <label className="block text-sm">
          {t("settings.endOfDay")}
          <input
            type="time"
            value={settings.end_of_day}
            onChange={(e) => set({ end_of_day: e.target.value })}
            className="mt-1 block min-h-8 rounded border border-line bg-bg p-1"
          />
        </label>
      </fieldset>

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">{t("alerts.title")}</legend>
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.alerts.charge_reminder}
            onChange={(e) =>
              set({
                alerts: { ...settings.alerts, charge_reminder: e.target.checked },
              })
            }
          />
          {t("alerts.chargeReminder")}
        </label>
        {settings.alerts.charge_reminder && (
          <label className="ml-6 flex min-h-8 items-center gap-2 text-sm">
            {t("alerts.atTarget")}
            <input
              type="number"
              min={1}
              max={100}
              value={settings.alerts.charge_target_percent}
              onChange={(e) =>
                set({
                  alerts: {
                    ...settings.alerts,
                    charge_target_percent: Number(e.target.value),
                  },
                })
              }
              className="min-h-8 w-16 rounded border border-line bg-bg p-1"
            />
            %
          </label>
        )}
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.alerts.low_battery}
            onChange={(e) =>
              set({
                alerts: { ...settings.alerts, low_battery: e.target.checked },
              })
            }
          />
          {t("alerts.lowBattery")}
        </label>
        {settings.alerts.low_battery && (
          <label className="ml-6 flex min-h-8 items-center gap-2 text-sm">
            {t("alerts.below")}
            <input
              type="number"
              min={1}
              max={100}
              value={settings.alerts.low_battery_percent}
              onChange={(e) =>
                set({
                  alerts: {
                    ...settings.alerts,
                    low_battery_percent: Number(e.target.value),
                  },
                })
              }
              className="min-h-8 w-16 rounded border border-line bg-bg p-1"
            />
            %
          </label>
        )}
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.alerts.overheat}
            onChange={(e) =>
              set({
                alerts: { ...settings.alerts, overheat: e.target.checked },
              })
            }
          />
          {t("alerts.overheat")}
        </label>
        {settings.alerts.overheat && (
          <label className="ml-6 flex min-h-8 items-center gap-2 text-sm">
            {t("alerts.above")}
            <input
              type="number"
              min={30}
              max={90}
              value={settings.alerts.overheat_celsius}
              onChange={(e) =>
                set({
                  alerts: {
                    ...settings.alerts,
                    overheat_celsius: Number(e.target.value),
                  },
                })
              }
              className="min-h-8 w-16 rounded border border-line bg-bg p-1"
            />
            °C
          </label>
        )}
        <label className="flex min-h-8 items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.alerts.timer_expired}
            onChange={(e) =>
              set({
                alerts: { ...settings.alerts, timer_expired: e.target.checked },
              })
            }
          />
          {t("alerts.timerExpired")}
        </label>
      </fieldset>

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">
          {t("settings.menuBarText")}
        </legend>
        <p className="text-xs text-ink-2">{t("settings.menuBarTextHelp")}</p>
        {(["countdown", "battery", "watts"] as const).map((metric) => {
          const selected = settings.menu_bar_metrics.includes(metric);
          return (
            <label
              key={metric}
              className="flex min-h-8 items-center gap-2 text-sm"
            >
              <input
                type="checkbox"
                checked={selected}
                disabled={!selected && settings.menu_bar_metrics.length >= 2}
                onChange={(e) =>
                  set({
                    menu_bar_metrics: e.target.checked
                      ? [...settings.menu_bar_metrics, metric]
                      : settings.menu_bar_metrics.filter((m) => m !== metric),
                  })
                }
              />
              {t(`settings.menuBarMetric.${metric}`)}
            </label>
          );
        })}
      </fieldset>

      <fieldset className="space-y-2">
        <legend className="text-sm font-semibold">
          {t("settings.appearance")}
        </legend>
        <label className="block text-sm">
          {t("settings.theme")}
          <select
            value={settings.theme}
            onChange={(e) => set({ theme: e.target.value })}
            className="mt-1 min-h-8 w-full rounded border border-line bg-bg p-1"
          >
            <option value="system">{t("settings.themeSystem")}</option>
            <option value="light">{t("settings.themeLight")}</option>
            <option value="dark">{t("settings.themeDark")}</option>
          </select>
        </label>
        <label className="block text-sm">
          {t("settings.language")}
          <select
            value={settings.language}
            onChange={(e) => set({ language: e.target.value })}
            className="mt-1 min-h-8 w-full rounded border border-line bg-bg p-1"
          >
            <option value="system">{t("settings.languageSystem")}</option>
            <option value="it">Italiano</option>
            <option value="en">English</option>
          </select>
        </label>
      </fieldset>

      <button
        type="button"
        onClick={onReopenOnboarding}
        className="min-h-8 rounded border border-line px-2 py-1 text-sm text-ink-2"
      >
        {t("settings.reopenOnboarding")}
      </button>
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
  const { t } = useTranslation();
  const setWindow = (i: number, patch: Partial<ScheduleWindow>) => {
    const windows = schedule.windows.map((w, j) =>
      j === i ? { ...w, ...patch } : w,
    );
    onChange({ ...schedule, windows });
  };

  return (
    <fieldset className="space-y-2">
      <legend className="text-sm font-semibold">
        {t("settings.schedule")}
      </legend>
      <label className="flex min-h-8 items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={schedule.enabled}
          onChange={(e) => onChange({ ...schedule, enabled: e.target.checked })}
        />
        {t("settings.scheduleEnabled")}
      </label>
      {schedule.enabled &&
        schedule.windows.map((w, i) => (
          <div
            key={i}
            className="ml-6 space-y-1 rounded border border-line p-2"
          >
            <div className="flex flex-wrap gap-1">
              {([0, 1, 2, 3, 4, 5, 6] as const).map((day) => (
                <button
                  key={day}
                  type="button"
                  aria-pressed={w.days.includes(day)}
                  aria-label={t("a11y.dayToggle", { day: t(`days.${day}`) })}
                  onClick={() =>
                    setWindow(i, {
                      days: w.days.includes(day)
                        ? w.days.filter((d) => d !== day)
                        : [...w.days, day].sort(),
                    })
                  }
                  className={`min-h-8 rounded px-2 py-0.5 text-xs ${
                    w.days.includes(day)
                      ? "bg-accent text-on-accent"
                      : "border border-line text-ink-2"
                  }`}
                >
                  {t(`days.${day}`)}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2 text-sm">
              <input
                type="time"
                value={w.start}
                aria-label={t("settings.windowStart")}
                onChange={(e) => setWindow(i, { start: e.target.value })}
                className="min-h-8 rounded border border-line bg-bg p-1"
              />
              <span aria-hidden="true">→</span>
              <input
                type="time"
                value={w.end}
                aria-label={t("settings.windowEnd")}
                onChange={(e) => setWindow(i, { end: e.target.value })}
                className="min-h-8 rounded border border-line bg-bg p-1"
              />
              <button
                type="button"
                onClick={() =>
                  onChange({
                    ...schedule,
                    windows: schedule.windows.filter((_, j) => j !== i),
                  })
                }
                className="ml-auto min-h-8 px-2 text-xs text-alert"
              >
                {t("settings.remove")}
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
          className="ml-6 min-h-8 rounded border border-line px-2 py-1 text-xs"
        >
          {t("settings.addWindow")}
        </button>
      )}
    </fieldset>
  );
}
