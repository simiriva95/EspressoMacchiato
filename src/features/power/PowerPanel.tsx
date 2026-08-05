// Energy & system panel (P4). Every metric that this hardware/OS doesn't
// expose says "not available" — no fake zeros, no mute dashes. Charts are
// interactive: hover for time+value, window selectable up to 8 h (all RAM).

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Segmented } from "../../components/Segmented";
import { formatSeconds } from "../../lib/format";
import {
  ipc,
  type DurationSerde,
  type PowerSample,
  type PowerSnapshot,
} from "../../lib/ipc";

export function PowerPanel() {
  const { t } = useTranslation();
  const [power, setPower] = useState<PowerSnapshot | null>(null);
  const [history, setHistory] = useState<PowerSample[]>([]);
  const [windowHours, setWindowHours] = useState<"2" | "4" | "8">("2");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.getPower().then(setPower);
    ipc.getPowerHistory().then(setHistory);
    ipc
      .onPowerSample(() => {
        ipc.getPower().then(setPower);
        ipc.getPowerHistory().then(setHistory);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  // Window relative to the newest sample (pure w.r.t. render).
  const newest = history.length > 0 ? history[history.length - 1].unix_ms : 0;
  const cutoff = newest - Number(windowHours) * 3600_000;
  const visible = history.filter((s) => s.unix_ms >= cutoff);

  return (
    <section
      aria-label={t("power.title")}
      className="card space-y-4 p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
          {t("power.title")}
        </h2>
        <Segmented
          value={windowHours}
          onChange={setWindowHours}
          label={t("power.window")}
          options={[
            { value: "2", label: "2h" },
            { value: "4", label: "4h" },
            { value: "8", label: "8h" },
          ]}
        />
      </div>

      <div>
        <div className="flex items-baseline gap-2">
          <span className="mono text-2xl" data-testid="battery-percent">
            {power?.percent != null
              ? `${Math.round(power.percent)}%`
              : t("power.notAvailable")}
          </span>
          {power && (
            <span className="text-sm text-ink-2">
              {power.on_ac ? t("power.onAc") : t("power.onBattery")}
            </span>
          )}
        </div>
        <Chart
          samples={visible}
          pick={(s) => s.percent}
          min={0}
          max={100}
          label={t("power.percentHistory")}
          format={(v) => `${v.toFixed(0)}%`}
        />
      </div>

      <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
        <Stat
          label={t("power.health")}
          value={power?.health_percent}
          format={(v) => `${v.toFixed(0)}%`}
        />
        <Stat label={t("power.cycles")} value={power?.cycle_count} />
        <Stat
          label={t("power.temperature")}
          value={power?.temperature_c}
          format={(v) => `${v.toFixed(1)} °C`}
        />
        <Stat
          label={t("power.voltage")}
          value={power?.voltage_v}
          format={(v) => `${v.toFixed(2)} V`}
        />
        <Stat
          label={t("power.watts")}
          value={power?.watts}
          format={(v) => `${v.toFixed(1)} W`}
        />
        <Stat
          label={
            power?.time_to_full != null
              ? t("power.timeToFull")
              : t("power.timeToEmpty")
          }
          value={duration(power?.time_to_full ?? power?.time_to_empty)}
          format={formatSeconds}
        />
      </dl>

      <MetricChart
        title={t("power.wattsHistory")}
        samples={visible}
        pick={(s) => s.watts}
        format={(v) => `${v.toFixed(1)} W`}
      />

      <div className="border-t border-line pt-3">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
          {t("power.system")}
        </h3>
        <dl className="mt-1 grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
          <Stat
            label={t("power.cpu")}
            value={power?.cpu_percent}
            format={(v) => `${v.toFixed(0)}%`}
          />
          <Stat
            label={t("power.memory")}
            value={power ? power.memory_used_bytes : null}
            format={(used) =>
              `${gib(used)} / ${gib(power?.memory_total_bytes ?? 0)} GiB`
            }
          />
          <Stat
            label={t("power.uptime")}
            value={power?.uptime_secs}
            format={formatSeconds}
          />
        </dl>

        <MetricChart
          title={t("power.cpuHistory")}
          samples={visible}
          pick={(s) => s.cpu_percent}
          min={0}
          max={100}
          format={(v) => `${v.toFixed(0)}%`}
        />
        <MetricChart
          title={t("power.tempHistory")}
          samples={visible}
          pick={(s) => s.temperature_c}
          format={(v) => `${v.toFixed(1)} °C`}
        />

        {power && power.top_energy_processes.length > 0 && (
          <div className="mt-2 text-sm">
            <h4 className="text-xs text-ink-2">{t("power.topProcesses")}</h4>
            <ul className="mt-1 space-y-0.5">
              {power.top_energy_processes.map((p) => (
                <li key={p.name} className="flex justify-between">
                  <span className="truncate">{p.name}</span>
                  <span className="mono text-ink-2">
                    {p.cpu_percent.toFixed(0)}%
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

function duration(d: DurationSerde | null | undefined): number | null {
  return d ? d.secs : null;
}

function gib(bytes: number): string {
  return (bytes / 1024 ** 3).toFixed(1);
}

function Stat<T extends number>({
  label,
  value,
  format,
}: {
  label: string;
  value: T | null | undefined;
  format?: (v: T) => string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <dt className="text-ink-2">{label}</dt>
      <dd className="mono">
        {value != null
          ? format
            ? format(value)
            : String(value)
          : t("power.notAvailable")}
      </dd>
    </>
  );
}

function MetricChart(props: {
  title: string;
  samples: PowerSample[];
  pick: (s: PowerSample) => number | null;
  min?: number;
  max?: number;
  format: (v: number) => string;
}) {
  const values = props.samples.map(props.pick);
  if (values.filter((v) => v != null).length < 2) return null;
  return (
    <div className="mt-3">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
        {props.title}
      </h3>
      <Chart {...props} label={props.title} />
    </div>
  );
}

/** Interactive history chart: hover (or focus + arrows) reads time+value. */
function Chart({
  samples,
  pick,
  min,
  max,
  label,
  format,
}: {
  samples: PowerSample[];
  pick: (s: PowerSample) => number | null;
  min?: number;
  max?: number;
  label: string;
  format: (v: number) => string;
}) {
  const [hover, setHover] = useState<number | null>(null);
  const values = samples.map(pick);
  const valid = values.filter((v): v is number => v != null);
  if (valid.length < 2) return null;

  const lo = min ?? Math.min(...valid);
  const hi = max ?? Math.max(...valid);
  const span = hi - lo || 1;
  const xOf = (i: number) => (i / (values.length - 1)) * 100;
  const yOf = (v: number) => 26 - ((v - lo) / span) * 22 - 2;
  const points = values
    .map((v, i) => (v == null ? null : `${xOf(i).toFixed(2)},${yOf(v).toFixed(2)}`))
    .filter(Boolean)
    .join(" ");

  const hoverSample =
    hover != null && values[hover] != null
      ? { value: values[hover] as number, time: samples[hover].unix_ms }
      : null;

  return (
    <div className="relative">
      {hoverSample && (
        <div
          aria-hidden="true"
          className="mono pointer-events-none absolute -top-1 z-10 -translate-x-1/2 rounded bg-surface-2 px-1.5 py-0.5 text-[10px] whitespace-nowrap"
          style={{ left: `${xOf(hover as number)}%` }}
        >
          {format(hoverSample.value)} ·{" "}
          {new Date(hoverSample.time).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          })}
        </div>
      )}
      <svg
        viewBox="0 0 100 26"
        preserveAspectRatio="none"
        role="img"
        aria-label={label}
        className="mt-1 h-8 w-full cursor-crosshair"
        onMouseMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const ratio = (e.clientX - rect.left) / rect.width;
          const index = Math.round(ratio * (values.length - 1));
          setHover(Math.max(0, Math.min(values.length - 1, index)));
        }}
        onMouseLeave={() => setHover(null)}
      >
        <polyline
          points={points}
          fill="none"
          stroke="var(--accent)"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
        {hoverSample && (
          <>
            <line
              x1={xOf(hover as number)}
              x2={xOf(hover as number)}
              y1="0"
              y2="26"
              stroke="var(--line)"
              vectorEffect="non-scaling-stroke"
            />
            <circle
              cx={xOf(hover as number)}
              cy={yOf(hoverSample.value)}
              r="1.6"
              fill="var(--accent)"
            />
          </>
        )}
      </svg>
    </div>
  );
}
