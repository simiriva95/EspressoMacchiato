// State word + activation controls. The display face appears here and only
// here: one occurrence per screen (spec §7).

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toDurationSecs, type DurationChoice } from "../../lib/duration";
import { ipc, type Settings, type StatusSnapshot } from "../../lib/ipc";

const STATE_COLOR: Record<StatusSnapshot["state"], string> = {
  off: "text-ink-2",
  active: "text-ok",
  suspended: "text-accent",
  degraded: "text-alert",
};

export function EngineControls({
  status,
  settings,
  compact = false,
}: {
  status: StatusSnapshot | null;
  settings: Settings | null;
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [duration, setDuration] = useState<DurationChoice>({
    kind: "indefinite",
  });
  const [untilTime, setUntilTime] = useState("17:00");

  const on = status !== null && status.state !== "off";
  const endOfDay = settings?.end_of_day ?? "18:00";

  const activate = () => {
    const choice: DurationChoice =
      duration.kind === "until" ? { kind: "until", time: untilTime } : duration;
    ipc.setActive(true, toDurationSecs(choice));
  };

  const detail = status
    ? t(`detail.${status.state_detail}`, {
        defaultValue: status.state_detail,
      })
    : "";

  return (
    <section aria-label={t("app.name")}>
      <div className="flex items-end justify-between gap-3">
        <div className="min-w-0">
          <p
            className={`display ${compact ? "text-2xl" : "text-3xl"} ${
              status ? STATE_COLOR[status.state] : "text-ink-2"
            }`}
          >
            {status ? t(`state.${status.state}`) : t("common.dash")}
          </p>
          <p className="truncate text-sm text-ink-2">
            {detail}
            {on &&
              status?.remaining_secs != null &&
              ` · ${t("state.remaining", {
                minutes: Math.max(1, Math.ceil(status.remaining_secs / 60)),
              })}`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => (on ? ipc.setActive(false) : activate())}
          className={`min-h-8 shrink-0 rounded px-4 py-2 font-semibold ${
            on
              ? "bg-alert-fill text-on-alert-fill"
              : "bg-accent text-on-accent"
          }`}
        >
          {on ? t("engine.deactivate") : t("engine.activate")}
        </button>
      </div>

      {!on && (
        <div className="mt-3 flex items-center gap-2 text-sm">
          <label htmlFor="duration" className="text-ink-2">
            {t("engine.duration")}
          </label>
          <select
            id="duration"
            className="min-h-8 rounded border border-line bg-surface p-1"
            value={
              duration.kind === "minutes"
                ? String(duration.minutes)
                : duration.kind === "until"
                  ? untilTime === endOfDay
                    ? "end_of_day"
                    : "until"
                  : "indefinite"
            }
            onChange={(e) => {
              const v = e.target.value;
              if (v === "indefinite") setDuration({ kind: "indefinite" });
              else if (v === "until")
                setDuration({ kind: "until", time: untilTime });
              else if (v === "end_of_day") {
                setUntilTime(endOfDay);
                setDuration({ kind: "until", time: endOfDay });
              } else setDuration({ kind: "minutes", minutes: Number(v) });
            }}
          >
            <option value="indefinite">{t("engine.indefinite")}</option>
            <option value="15">{t("engine.minutes", { count: 15 })}</option>
            <option value="30">{t("engine.minutes", { count: 30 })}</option>
            <option value="60">{t("engine.hours", { count: 1 })}</option>
            <option value="120">{t("engine.hours", { count: 2 })}</option>
            <option value="240">{t("engine.hours", { count: 4 })}</option>
            <option value="until">{t("engine.untilTime")}</option>
            <option value="end_of_day">
              {t("engine.untilEndOfDay", { time: endOfDay })}
            </option>
          </select>
          {duration.kind === "until" && untilTime !== endOfDay && (
            <input
              type="time"
              aria-label={t("engine.untilTime")}
              value={untilTime}
              onChange={(e) => setUntilTime(e.target.value)}
              className="min-h-8 rounded border border-line bg-surface p-1"
            />
          )}
        </div>
      )}
    </section>
  );
}
