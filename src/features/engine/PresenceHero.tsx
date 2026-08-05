// The Presence section hero: big state word, prominent activation, live
// idle ring. This is the face of the app.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toDurationSecs, type DurationChoice } from "../../lib/duration";
import { formatSeconds } from "../../lib/format";
import { ipc, type Settings, type StatusSnapshot } from "../../lib/ipc";

const RING_COLOR: Record<StatusSnapshot["state"], string> = {
  off: "var(--ink-2)",
  active: "var(--ok)",
  suspended: "var(--accent)",
  degraded: "var(--alert)",
};

export function PresenceHero({
  status,
  settings,
  pokeSignal,
}: {
  status: StatusSnapshot | null;
  settings: Settings | null;
  pokeSignal: number;
}) {
  const { t } = useTranslation();
  const [duration, setDuration] = useState<DurationChoice>({
    kind: "indefinite",
  });
  const [untilTime, setUntilTime] = useState("17:00");
  const [meetingEnd, setMeetingEnd] = useState<number | null>(null);
  const [idle, setIdle] = useState<number | null>(null);
  const [flashing, setFlashing] = useState(false);

  useEffect(() => {
    const poll = () => ipc.getIdleSeconds().then(setIdle, () => setIdle(null));
    poll();
    const timer = window.setInterval(poll, 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (pokeSignal === 0) return;
    const a = window.setTimeout(() => {
      setIdle(0);
      setFlashing(true);
    }, 0);
    const b = window.setTimeout(() => setFlashing(false), 700);
    return () => {
      window.clearTimeout(a);
      window.clearTimeout(b);
    };
  }, [pokeSignal]);

  const on = status !== null && status.state !== "off";
  const endOfDay = settings?.end_of_day ?? "18:00";
  const interval = status?.interval_secs ?? 60;
  const ringFraction =
    idle !== null ? Math.min(1, idle / interval) : 0;

  const activate = () => {
    const choice: DurationChoice =
      duration.kind === "until" ? { kind: "until", time: untilTime } : duration;
    ipc.setActive(true, toDurationSecs(choice));
  };

  const detail = status
    ? t(`detail.${status.state_detail}`, { defaultValue: status.state_detail })
    : "";

  // SVG ring geometry.
  const R = 74;
  const C = 2 * Math.PI * R;

  return (
    <section
      aria-label={t("app.name")}
      className="card flex flex-col items-center gap-6 p-8 sm:flex-row sm:items-center sm:gap-10"
    >
      {/* Idle ring with the cup at center. */}
      <div className="relative shrink-0" style={{ width: 180, height: 180 }}>
        <svg viewBox="0 0 180 180" className="h-full w-full -rotate-90">
          <circle
            cx="90"
            cy="90"
            r={R}
            fill="none"
            stroke="var(--line)"
            strokeWidth="6"
          />
          <circle
            cx="90"
            cy="90"
            r={R}
            fill="none"
            stroke={status ? RING_COLOR[status.state] : "var(--ink-2)"}
            strokeWidth="6"
            strokeLinecap="round"
            strokeDasharray={C}
            strokeDashoffset={C * (1 - ringFraction)}
            style={{ transition: "stroke-dashoffset 0.9s linear" }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <div
            className={`mono text-3xl ${flashing ? "poke-flash" : ""}`}
            data-testid="idle-value"
          >
            {idle !== null ? formatSeconds(idle) : t("common.dash")}
          </div>
          <div className="text-[10px] uppercase tracking-widest text-ink-2">
            {t("idle.title")}
          </div>
        </div>
      </div>

      {/* State + controls. */}
      <div className="flex-1 text-center sm:text-left">
        <p
          className="display text-4xl leading-none sm:text-5xl"
          style={{ color: status ? RING_COLOR[status.state] : "var(--ink-2)" }}
        >
          {status ? t(`state.${status.state}`) : t("common.dash")}
        </p>
        <p className="mt-2 text-sm text-ink-2">
          {detail}
          {on &&
            status?.remaining_secs != null &&
            ` · ${t("state.remaining", {
              minutes: Math.max(1, Math.ceil(status.remaining_secs / 60)),
            })}`}
          {" · "}
          {t("idle.sourcePrefix")} {status?.idle_source ?? "—"}
        </p>

        <div className="mt-5 flex flex-wrap items-center justify-center gap-3 sm:justify-start">
          <button
            type="button"
            onClick={() => (on ? ipc.setActive(false) : activate())}
            className={`min-h-11 rounded-full px-7 text-base font-semibold shadow-lg transition-transform hover:-translate-y-0.5 ${
              on
                ? "bg-alert-fill text-on-alert-fill"
                : "bg-accent text-on-accent"
            }`}
          >
            {on ? t("engine.deactivate") : t("engine.activate")}
          </button>

          {!on && (
            <select
              aria-label={t("engine.duration")}
              className="min-h-11 rounded-full border border-line bg-surface-2 px-4"
              onFocus={() => {
                if (meetingEnd === null)
                  ipc
                    .getNextMeetingEnd()
                    .then(setMeetingEnd)
                    .catch(() => setMeetingEnd(null));
              }}
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
                } else if (v === "meeting" && meetingEnd !== null)
                  setDuration({ kind: "epoch", endMs: meetingEnd * 1000 });
                else setDuration({ kind: "minutes", minutes: Number(v) });
              }}
            >
              <option value="indefinite">{t("engine.indefinite")}</option>
              <option value="25">{t("engine.espressoShot")}</option>
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
          )}

          {duration.kind === "until" && untilTime !== endOfDay && !on && (
            <input
              type="time"
              aria-label={t("engine.untilTime")}
              value={untilTime}
              onChange={(e) => setUntilTime(e.target.value)}
              className="min-h-11 rounded-full border border-line bg-surface-2 px-3"
            />
          )}

          <button
            type="button"
            onClick={() => ipc.pokeNow()}
            className="min-h-11 rounded-full border border-line px-5 text-sm text-ink-2 hover:text-ink"
          >
            {t("idle.testNow")}
          </button>
        </div>
      </div>
    </section>
  );
}
