// Quick panel anchored under the macOS tray icon: presence switch, the
// live idle counter, battery at a glance, one link to Settings.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { StateAnnouncer } from "../components/StateAnnouncer";
import { EngineControls } from "../features/engine/EngineControls";
import { IdleMonitor } from "../features/idle-monitor/IdleMonitor";
import { BatteryStrip } from "../features/power/BatteryStrip";
import { ipc, type StatsReport } from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

export function Popover() {
  const { t } = useTranslation();
  const { status, settings, pokeSignal } = useEngine();
  const [stats, setStats] = useState<StatsReport | null>(null);

  useEffect(() => {
    ipc.getStats().then(setStats, () => setStats(null));
  }, [status?.state]);

  return (
    <main className="flex h-screen flex-col gap-3 overflow-y-auto bg-bg p-4">
      <StateAnnouncer status={status} />
      <div className="card p-3">
        <EngineControls status={status} settings={settings} />
        {stats !== null && stats.shots_today > 0 && (
          <p className="mono mt-2 text-xs text-ink-2" title={t("report.shots")}>
            ☕ ×{stats.shots_today}
          </p>
        )}
      </div>
      <IdleMonitor status={status} pokeSignal={pokeSignal} compact />
      <BatteryStrip />
      <button
        type="button"
        onClick={() => ipc.openSettingsWindow().catch(() => {})}
        className="mt-auto min-h-8 rounded-full border border-line px-3 py-1.5 text-sm text-ink-2 hover:text-ink"
      >
        {t("popover.openSettings")}
      </button>
    </main>
  );
}
