// Compact battery line for the popover: percent, power source, draw.
// The app is an energy dashboard too, not just a keep-awake switch.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ipc, type PowerSnapshot } from "../../lib/ipc";

export function BatteryStrip() {
  const { t } = useTranslation();
  const [power, setPower] = useState<PowerSnapshot | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.getPower().then(setPower);
    ipc
      .onPowerSample(() => {
        ipc.getPower().then(setPower);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  if (!power || power.percent == null) return null;

  const fill = Math.max(0, Math.min(100, power.percent));

  return (
    <section aria-label={t("power.title")} className="card p-3">
      <div className="flex items-baseline justify-between gap-2 text-sm">
        <span className="mono text-lg">{Math.round(fill)}%</span>
        <span className="truncate text-xs text-ink-2">
          {power.on_ac ? t("power.onAc") : t("power.onBattery")}
          {power.watts != null && ` · ${power.watts.toFixed(1)} W`}
          {!power.on_ac &&
            power.time_to_empty != null &&
            ` · ${Math.round(power.time_to_empty.secs / 60)}m`}
        </span>
      </div>
      <div aria-hidden="true" className="mt-2 h-1 w-full rounded bg-line">
        <div
          className={`h-1 rounded ${fill <= 20 ? "bg-alert-fill" : "bg-ok"}`}
          style={{ width: `${fill}%` }}
        />
      </div>
    </section>
  );
}
