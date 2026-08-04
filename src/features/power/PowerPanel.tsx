// Energy & system panel (P4). Every metric that this hardware/OS doesn't
// expose says "not available" — no fake zeros, no mute dashes.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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

  return (
    <section
      aria-label={t("power.title")}
      className="card space-y-4 p-4"
    >
      <h2 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
        {t("power.title")}
      </h2>

      <div>
        <div className="flex items-baseline gap-2">
          <span className="mono text-4xl" data-testid="battery-percent">
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
        <Sparkline
          values={history.map((s) => s.percent)}
          min={0}
          max={100}
          label={t("power.percentHistory")}
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

      <div>
        <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
          {t("power.wattsHistory")}
        </h3>
        <Sparkline
          values={history.map((s) => s.watts)}
          label={t("power.wattsHistory")}
        />
      </div>

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

/** Hand-rolled sparkline: last 2h of one metric, gaps for missing values. */
function Sparkline({
  values,
  min,
  max,
  label,
}: {
  values: (number | null)[];
  min?: number;
  max?: number;
  label: string;
}) {
  const valid = values.filter((v): v is number => v != null);
  if (valid.length < 2) return null;
  const lo = min ?? Math.min(...valid);
  const hi = max ?? Math.max(...valid);
  const span = hi - lo || 1;
  const points = values
    .map((v, i) =>
      v == null
        ? null
        : `${((i / (values.length - 1)) * 100).toFixed(2)},${(
            24 -
            ((v - lo) / span) * 22 -
            1
          ).toFixed(2)}`,
    )
    .filter(Boolean)
    .join(" ");
  return (
    <svg
      viewBox="0 0 100 24"
      preserveAspectRatio="none"
      role="img"
      aria-label={label}
      className="mt-1 h-6 w-full"
    >
      <polyline
        points={points}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
