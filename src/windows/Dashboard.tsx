// The main window: a wide dashboard with a vertical nav rail and a content
// pane. Sections: Presence, Energy, Activity, Settings.

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DegradationsCard } from "../components/DegradationsCard";
import { StateAnnouncer } from "../components/StateAnnouncer";
import {
  BoltIcon,
  ChartIcon,
  CupIcon,
  SlidersIcon,
} from "../components/icons";
import { ActivityPanel } from "../features/activity/ActivityPanel";
import { PresenceHero } from "../features/engine/PresenceHero";
import { Onboarding } from "../features/onboarding/Onboarding";
import { PowerPanel } from "../features/power/PowerPanel";
import { SettingsForm } from "../features/settings/SettingsForm";
import { ipc } from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

type Section = "presence" | "energy" | "activity" | "settings";

const STATE_DOT: Record<string, string> = {
  off: "bg-ink-2",
  active: "bg-ok",
  suspended: "bg-accent",
  degraded: "bg-alert",
};

export function Dashboard() {
  const { t } = useTranslation();
  const { status, settings, save, saveError, pokeSignal } = useEngine();
  const [section, setSection] = useState<Section>(() => {
    // Dev preview convenience: ?section=energy. No effect inside Tauri.
    const q = new URLSearchParams(window.location.search).get("section");
    return (["presence", "energy", "activity", "settings"] as const).includes(
      q as Section,
    )
      ? (q as Section)
      : "presence";
  });
  const [showOnboarding, setShowOnboarding] = useState(false);
  const onboardingChecked = useRef(false);

  useEffect(() => {
    if (!settings || onboardingChecked.current) return;
    onboardingChecked.current = true;
    if (!settings.onboarding_done) {
      ipc.getPermissionStatus().then((degradations) => {
        if (degradations.length > 0) setShowOnboarding(true);
        else save({ ...settings, onboarding_done: true });
      });
    }
  }, [settings, save]);

  const nav: { id: Section; label: string; Icon: typeof CupIcon }[] = [
    { id: "presence", label: t("nav.presence"), Icon: CupIcon },
    { id: "energy", label: t("nav.energy"), Icon: BoltIcon },
    { id: "activity", label: t("nav.activity"), Icon: ChartIcon },
    { id: "settings", label: t("nav.settings"), Icon: SlidersIcon },
  ];

  return (
    <div className="flex h-screen overflow-hidden bg-bg text-ink">
      <StateAnnouncer status={status} />

      {/* Nav rail */}
      <nav
        aria-label={t("app.name")}
        data-tauri-drag-region
        className="flex w-56 shrink-0 flex-col gap-1 border-r border-line p-4"
      >
        <div
          data-tauri-drag-region
          className="mb-4 flex items-center gap-2 px-2 pt-1"
        >
          <CupIcon className="h-6 w-6 text-accent" />
          <span className="display text-lg">EspressoMacchiato</span>
        </div>
        {nav.map(({ id, label, Icon }) => (
          <button
            key={id}
            type="button"
            aria-current={section === id}
            onClick={() => setSection(id)}
            className={`flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-colors ${
              section === id
                ? "bg-surface font-semibold text-ink"
                : "text-ink-2 hover:bg-surface-2 hover:text-ink"
            }`}
          >
            <Icon />
            {label}
          </button>
        ))}

        {/* Live status footer */}
        <div className="mt-auto flex items-center gap-2 rounded-xl border border-line px-3 py-2.5 text-xs">
          <span
            className={`h-2.5 w-2.5 rounded-full ${
              status ? STATE_DOT[status.state] : "bg-ink-2"
            }`}
          />
          <span className="text-ink-2">
            {status ? t(`state.${status.state}`) : "…"}
          </span>
        </div>
      </nav>

      {/* Content */}
      <main className="flex-1 overflow-y-auto p-6">
        <div key={section} className="rise-in mx-auto max-w-4xl space-y-5">
          <DegradationsCard degradations={status?.degradations ?? []} />

          {section === "presence" && (
            <PresenceHero
              status={status}
              settings={settings}
              pokeSignal={pokeSignal}
            />
          )}
          {section === "energy" && <PowerPanel />}
          {section === "activity" && <ActivityPanel />}
          {section === "settings" && settings && (
            <SettingsForm
              settings={settings}
              status={status}
              onChange={save}
              saveError={saveError}
              onReopenOnboarding={() => setShowOnboarding(true)}
            />
          )}
        </div>
      </main>

      {showOnboarding && settings && (
        <Onboarding
          settings={settings}
          onSave={save}
          onClose={() => setShowOnboarding(false)}
        />
      )}
    </div>
  );
}
