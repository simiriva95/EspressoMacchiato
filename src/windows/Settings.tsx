// The settings window: grouped glass cards, switch rows, segmented
// controls. Full controls + idle monitor + energy panel + first-run wizard.

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DegradationsCard } from "../components/DegradationsCard";
import { Row } from "../components/Row";
import { Segmented } from "../components/Segmented";
import { StateAnnouncer } from "../components/StateAnnouncer";
import { Toggle } from "../components/Toggle";
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

      <div className="card p-4">
        <EngineControls status={status} settings={settings} />
      </div>
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

function Card({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section aria-label={title} className="card p-4">
      <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-2">
        {title}
      </h2>
      <div className="hairline-rows">{children}</div>
    </section>
  );
}

function NumberField({
  value,
  onChange,
  min,
  max,
  label,
  suffix,
}: {
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  label: string;
  suffix: string;
}) {
  return (
    <span className="flex items-center gap-1 text-sm text-ink-2">
      <input
        type="number"
        aria-label={label}
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="min-h-8 w-16 rounded-lg border border-line bg-bg p-1 text-ink"
      />
      {suffix}
    </span>
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
  const setAlerts = (patch: Partial<Settings["alerts"]>) =>
    set({ alerts: { ...settings.alerts, ...patch } });

  return (
    <div className="space-y-4">
      {saveError && (
        <p
          className="card border-alert p-3 text-xs text-alert"
          role="alert"
        >
          {t("common.saveError", { error: saveError })}
        </p>
      )}

      <Card title={t("settings.title")}>
        <Row
          label={
            <>
              {t("settings.interval")}{" "}
              <span className="mono">{settings.interval_secs}s</span>
            </>
          }
          description={t("settings.intervalHelp")}
          stacked
        >
          <input
            type="range"
            aria-label={t("settings.interval")}
            min={10}
            max={240}
            step={5}
            value={settings.interval_secs}
            onChange={(e) => set({ interval_secs: Number(e.target.value) })}
            className="w-full"
          />
        </Row>
        <Row label={t("settings.strategy")} stacked>
          <select
            aria-label={t("settings.strategy")}
            value={settings.strategy}
            onChange={(e) =>
              set({ strategy: e.target.value as ActivityStrategy })
            }
            className="min-h-8 w-full rounded-lg border border-line bg-bg p-1.5"
          >
            {(status?.available_strategies ?? [settings.strategy]).map((s) => (
              <option key={s} value={s}>
                {t(`settings.strategies.${s}`)}
              </option>
            ))}
          </select>
        </Row>
      </Card>

      <Card title={t("settings.conditions")}>
        <Row label={t("settings.pauseWhenInputRecent")}>
          <Toggle
            checked={settings.conditions.pause_when_input_recent}
            onChange={(v) => setConditions({ pause_when_input_recent: v })}
            label={t("settings.pauseWhenInputRecent")}
          />
        </Row>
        <Row label={t("settings.onlyOnAc")}>
          {settings.conditions.only_on_ac && (
            <NumberField
              value={settings.conditions.min_battery_percent ?? 100}
              onChange={(v) =>
                setConditions({
                  min_battery_percent: v >= 100 ? null : v,
                })
              }
              min={1}
              max={100}
              label={t("settings.onlyBelow")}
              suffix="%"
            />
          )}
          <Toggle
            checked={settings.conditions.only_on_ac}
            onChange={(v) => setConditions({ only_on_ac: v })}
            label={t("settings.onlyOnAc")}
          />
        </Row>
        <Row
          label={t("settings.onlyWhenProcessRunning")}
          description={
            settings.conditions.only_when_process_running ? (
              <input
                type="text"
                aria-label={t("settings.processNames")}
                value={settings.conditions.process_names.join(", ")}
                onChange={(e) =>
                  setConditions({
                    process_names: e.target.value
                      .split(",")
                      .map((s) => s.trim()),
                  })
                }
                className="mt-1 min-h-8 w-full rounded-lg border border-line bg-bg p-1.5 text-sm text-ink"
              />
            ) : undefined
          }
        >
          <Toggle
            checked={settings.conditions.only_when_process_running}
            onChange={(v) => setConditions({ only_when_process_running: v })}
            label={t("settings.onlyWhenProcessRunning")}
          />
        </Row>
        <Row label={t("settings.pauseWhenScreenLocked")}>
          <Toggle
            checked={settings.conditions.pause_when_screen_locked}
            onChange={(v) => setConditions({ pause_when_screen_locked: v })}
            label={t("settings.pauseWhenScreenLocked")}
          />
        </Row>
      </Card>

      <Card title={t("settings.schedule")}>
        <ScheduleEditor
          schedule={settings.schedule}
          onChange={(schedule) => set({ schedule })}
        />
      </Card>

      <Card title={t("alerts.title")}>
        <Row label={t("alerts.chargeReminder")}>
          {settings.alerts.charge_reminder && (
            <NumberField
              value={settings.alerts.charge_target_percent}
              onChange={(v) => setAlerts({ charge_target_percent: v })}
              min={1}
              max={100}
              label={t("alerts.atTarget")}
              suffix="%"
            />
          )}
          <Toggle
            checked={settings.alerts.charge_reminder}
            onChange={(v) => setAlerts({ charge_reminder: v })}
            label={t("alerts.chargeReminder")}
          />
        </Row>
        <Row label={t("alerts.lowBattery")}>
          {settings.alerts.low_battery && (
            <NumberField
              value={settings.alerts.low_battery_percent}
              onChange={(v) => setAlerts({ low_battery_percent: v })}
              min={1}
              max={100}
              label={t("alerts.below")}
              suffix="%"
            />
          )}
          <Toggle
            checked={settings.alerts.low_battery}
            onChange={(v) => setAlerts({ low_battery: v })}
            label={t("alerts.lowBattery")}
          />
        </Row>
        <Row label={t("alerts.overheat")}>
          {settings.alerts.overheat && (
            <NumberField
              value={settings.alerts.overheat_celsius}
              onChange={(v) => setAlerts({ overheat_celsius: v })}
              min={30}
              max={90}
              label={t("alerts.above")}
              suffix="°C"
            />
          )}
          <Toggle
            checked={settings.alerts.overheat}
            onChange={(v) => setAlerts({ overheat: v })}
            label={t("alerts.overheat")}
          />
        </Row>
        <Row label={t("alerts.timerExpired")}>
          <Toggle
            checked={settings.alerts.timer_expired}
            onChange={(v) => setAlerts({ timer_expired: v })}
            label={t("alerts.timerExpired")}
          />
        </Row>
      </Card>

      <Card title={t("settings.startup")}>
        <Row label={t("settings.autostart")}>
          <Toggle
            checked={settings.autostart}
            onChange={(v) => set({ autostart: v })}
            label={t("settings.autostart")}
          />
        </Row>
        <Row label={t("settings.activateOnStart")}>
          <Toggle
            checked={settings.activate_on_start}
            onChange={(v) => set({ activate_on_start: v })}
            label={t("settings.activateOnStart")}
          />
        </Row>
        <Row label={t("settings.hotkey")} stacked>
          <input
            type="text"
            aria-label={t("settings.hotkey")}
            value={settings.hotkey}
            onChange={(e) => set({ hotkey: e.target.value })}
            className="mono min-h-8 w-full rounded-lg border border-line bg-bg p-1.5"
            placeholder={t("settings.hotkeyPlaceholder")}
          />
        </Row>
        <Row label={t("settings.endOfDay")}>
          <input
            type="time"
            aria-label={t("settings.endOfDay")}
            value={settings.end_of_day}
            onChange={(e) => set({ end_of_day: e.target.value })}
            className="min-h-8 rounded-lg border border-line bg-bg p-1"
          />
        </Row>
      </Card>

      <Card title={t("settings.menuBarText")}>
        <Row label={t("settings.menuBarTextHelp")} stacked>
          <div className="space-y-2">
            {(["countdown", "battery", "watts"] as const).map((metric) => {
              const selected = settings.menu_bar_metrics.includes(metric);
              return (
                <div
                  key={metric}
                  className="flex items-center justify-between gap-4"
                >
                  <span className="text-sm">
                    {t(`settings.menuBarMetric.${metric}`)}
                  </span>
                  <Toggle
                    checked={selected}
                    disabled={!selected && settings.menu_bar_metrics.length >= 2}
                    onChange={(v) =>
                      set({
                        menu_bar_metrics: v
                          ? [...settings.menu_bar_metrics, metric]
                          : settings.menu_bar_metrics.filter(
                              (m) => m !== metric,
                            ),
                      })
                    }
                    label={t(`settings.menuBarMetric.${metric}`)}
                  />
                </div>
              );
            })}
          </div>
        </Row>
      </Card>

      <Card title={t("settings.appearance")}>
        <Row label={t("settings.theme")}>
          <Segmented
            value={settings.theme}
            onChange={(v) => set({ theme: v })}
            label={t("settings.theme")}
            options={[
              { value: "system", label: t("settings.themeSystem") },
              { value: "dark", label: t("settings.themeDark") },
              { value: "light", label: t("settings.themeLight") },
            ]}
          />
        </Row>
        <Row label={t("settings.language")}>
          <Segmented
            value={settings.language}
            onChange={(v) => set({ language: v })}
            label={t("settings.language")}
            options={[
              { value: "system", label: t("settings.languageSystem") },
              { value: "it", label: "IT" },
              { value: "en", label: "EN" },
            ]}
          />
        </Row>
        <Row label={t("settings.reopenOnboarding")}>
          <button
            type="button"
            onClick={onReopenOnboarding}
            className="min-h-8 rounded-full border border-line px-3 py-1 text-sm text-ink-2 hover:text-ink"
          >
            {t("onboarding.welcomeTitle")}
          </button>
        </Row>
      </Card>
    </div>
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
    <>
      <Row label={t("settings.scheduleEnabled")}>
        <Toggle
          checked={schedule.enabled}
          onChange={(v) => onChange({ ...schedule, enabled: v })}
          label={t("settings.scheduleEnabled")}
        />
      </Row>
      {schedule.enabled &&
        schedule.windows.map((w, i) => (
          <div key={i} className="space-y-2 py-3">
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
                  className={`min-h-8 rounded-full px-2.5 py-0.5 text-xs transition-colors ${
                    w.days.includes(day)
                      ? "bg-accent font-semibold text-on-accent"
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
                className="min-h-8 rounded-lg border border-line bg-bg p-1"
              />
              <span aria-hidden="true">→</span>
              <input
                type="time"
                value={w.end}
                aria-label={t("settings.windowEnd")}
                onChange={(e) => setWindow(i, { end: e.target.value })}
                className="min-h-8 rounded-lg border border-line bg-bg p-1"
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
        <div className="py-3">
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
            className="min-h-8 rounded-full border border-line px-3 py-1 text-xs text-ink-2 hover:text-ink"
          >
            {t("settings.addWindow")}
          </button>
        </div>
      )}
    </>
  );
}
