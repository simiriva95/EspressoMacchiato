// Status chip + main power switch + duration. The keep-awake control is
// one row among the app's panels now, not a shouting headline.

import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Toggle } from "../../components/Toggle";
import { toDurationSecs, type DurationChoice } from "../../lib/duration";
import { ipc, type Settings, type StatusSnapshot } from "../../lib/ipc";

const DOT_COLOR: Record<StatusSnapshot["state"], string> = {
  off: "bg-ink-2",
  active: "bg-ok",
  suspended: "bg-accent",
  degraded: "bg-alert",
};

const TEXT_COLOR: Record<StatusSnapshot["state"], string> = {
  off: "text-ink-2",
  active: "text-ok",
  suspended: "text-accent",
  degraded: "text-alert",
};

export function EngineControls({
  status,
  settings,
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
  // "Until end of meeting": looked up lazily when the dropdown is opened,
  // so the system Calendar permission dialog never fires in the background.
  const [meetingEnd, setMeetingEnd] = useState<number | null>(null);
  const meetingChecked = useRef(false);

  const lookUpMeeting = () => {
    if (meetingChecked.current) return;
    meetingChecked.current = true;
    ipc
      .getNextMeetingEnd()
      .then(setMeetingEnd)
      .catch(() => setMeetingEnd(null)); // denied/unavailable → option hidden
  };

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
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <span className={`chip ${status ? TEXT_COLOR[status.state] : "text-ink-2"}`}>
            <span
              aria-hidden="true"
              className={`h-2 w-2 rounded-full ${
                status ? DOT_COLOR[status.state] : "bg-ink-2"
              }`}
            />
            {status ? t(`state.${status.state}`) : t("common.dash")}
          </span>
          <p className="mt-1.5 truncate text-xs text-ink-2">
            {detail}
            {on &&
              status?.remaining_secs != null &&
              ` · ${t("state.remaining", {
                minutes: Math.max(1, Math.ceil(status.remaining_secs / 60)),
              })}`}
          </p>
        </div>
        <Toggle
          checked={on}
          onChange={(next) => (next ? activate() : ipc.setActive(false))}
          label={on ? t("engine.deactivate") : t("engine.activate")}
        />
      </div>

      {!on && (
        <div className="mt-3 flex items-center gap-2 text-sm">
          <label htmlFor="duration" className="text-ink-2">
            {t("engine.duration")}
          </label>
          <select
            id="duration"
            className="min-h-8 flex-1 rounded-full border border-line bg-surface-2 px-3 py-1"
            onFocus={lookUpMeeting}
            value={
              duration.kind === "minutes"
                ? String(duration.minutes)
                : duration.kind === "epoch"
                  ? "meeting"
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
              } else if (v === "meeting" && meetingEnd !== null) {
                setDuration({ kind: "epoch", endMs: meetingEnd * 1000 });
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
            {meetingEnd !== null && (
              <option value="meeting">
                {t("engine.untilMeeting", {
                  time: new Date(meetingEnd * 1000).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  }),
                })}
              </option>
            )}
          </select>
          {duration.kind === "until" && untilTime !== endOfDay && (
            <input
              type="time"
              aria-label={t("engine.untilTime")}
              value={untilTime}
              onChange={(e) => setUntilTime(e.target.value)}
              className="min-h-8 rounded-full border border-line bg-surface-2 px-2 py-1"
            />
          )}
        </div>
      )}
    </section>
  );
}
