// The Presence section hero: an animated brewing cup whose coffee level is
// the live idle progress, the big state word, prominent activation, and a
// mini stat strip. This is the face of the app.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toDurationSecs, type DurationChoice } from "../../lib/duration";
import { formatSeconds } from "../../lib/format";
import {
  ipc,
  type PowerSnapshot,
  type Settings,
  type StatusSnapshot,
} from "../../lib/ipc";
import { BrewingCup } from "./BrewingCup";

const STATE_COLOR: Record<StatusSnapshot["state"], string> = {
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
  const [power, setPower] = useState<PowerSnapshot | null>(null);

  useEffect(() => {
    const poll = () => ipc.getIdleSeconds().then(setIdle, () => setIdle(null));
    poll();
    const timer = window.setInterval(poll, 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    ipc.getPower().then(setPower, () => setPower(null));
    let unlisten: (() => void) | undefined;
    ipc
      .onPowerSample(() => ipc.getPower().then(setPower, () => {}))
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
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
  const fill = idle !== null ? Math.min(1, idle / interval) : 0;
  const color = status ? STATE_COLOR[status.state] : "var(--ink-2)";

  const activate = () => {
    const choice: DurationChoice =
      duration.kind === "until" ? { kind: "until", time: untilTime } : duration;
    ipc.setActive(true, toDurationSecs(choice));
  };

  const detail = status
    ? t(`detail.${status.state_detail}`, { defaultValue: status.state_detail })
    : "";

  return (
    <section aria-label={t("app.name")} className="card p-8">
      <div className="flex flex-col items-center gap-8 sm:flex-row sm:items-center sm:gap-10">
        {/* Animated brewing carafe */}
        <div
          className={`shrink-0 ${flashing ? "poke-flash" : ""}`}
          style={{ width: 176, height: 176 }}
        >
          <BrewingCup active={on} fill={fill} color={color} />
        </div>

        {/* State + controls */}
        <div className="flex-1 text-center sm:text-left">
          <p
            className="display text-4xl leading-none sm:text-5xl"
            style={{ color }}
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
      </div>

      {/* Mini stat strip */}
      <div className="mt-7 grid grid-cols-4 gap-3 border-t border-line pt-5 text-center">
        <MiniStat
          label={t("idle.title")}
          value={idle !== null ? formatSeconds(idle) : t("common.dash")}
          testId="idle-value"
        />
        <MiniStat
          label={t("idle.nextPoke")}
          value={
            status?.next_poke_in_secs != null
              ? formatSeconds(status.next_poke_in_secs)
              : "—"
          }
        />
        <MiniStat
          label={t("idle.pokes")}
          value={String(status?.poke_count ?? 0)}
        />
        <MiniStat
          label={t("power.title")}
          value={power?.percent != null ? `${Math.round(power.percent)}%` : "—"}
        />
      </div>
    </section>
  );
}

function MiniStat({
  label,
  value,
  testId,
}: {
  label: string;
  value: string;
  testId?: string;
}) {
  return (
    <div>
      <div className="mono text-xl" data-testid={testId}>
        {value}
      </div>
      <div className="mt-0.5 text-[11px] uppercase tracking-widest text-ink-2">
        {label}
      </div>
    </div>
  );
}
