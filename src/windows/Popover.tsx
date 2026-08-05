// Quick panel anchored under the macOS tray icon: presence switch, the
// live idle counter, battery at a glance, one link to Settings.

import { useTranslation } from "react-i18next";
import { StateAnnouncer } from "../components/StateAnnouncer";
import { EngineControls } from "../features/engine/EngineControls";
import { IdleMonitor } from "../features/idle-monitor/IdleMonitor";
import { BatteryStrip } from "../features/power/BatteryStrip";
import { ipc } from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

export function Popover() {
  const { t } = useTranslation();
  const { status, settings, pokeSignal } = useEngine();

  return (
    <main className="flex h-screen flex-col gap-3 overflow-y-auto bg-bg p-4">
      <StateAnnouncer status={status} />
      <div className="card p-3">
        <EngineControls status={status} settings={settings} />
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
