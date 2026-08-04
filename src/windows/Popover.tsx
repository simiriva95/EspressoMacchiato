// Quick panel anchored under the macOS tray icon. Compact by design:
// state, toggle, the signature counter, one link to Settings.

import { useTranslation } from "react-i18next";
import { StateAnnouncer } from "../components/StateAnnouncer";
import { EngineControls } from "../features/engine/EngineControls";
import { IdleMonitor } from "../features/idle-monitor/IdleMonitor";
import { ipc } from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

export function Popover() {
  const { t } = useTranslation();
  const { status, settings, pokeSignal } = useEngine();

  return (
    <main className="flex h-screen flex-col gap-3 overflow-y-auto border border-line bg-bg p-4">
      <StateAnnouncer status={status} />
      <EngineControls status={status} settings={settings} compact />
      <IdleMonitor status={status} pokeSignal={pokeSignal} compact />
      <button
        type="button"
        onClick={() => ipc.openSettingsWindow().catch(() => {})}
        className="mt-auto min-h-8 rounded border border-line px-3 py-1.5 text-sm text-ink-2 hover:bg-surface"
      >
        {t("popover.openSettings")}
      </button>
    </main>
  );
}
