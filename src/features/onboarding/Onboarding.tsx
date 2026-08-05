// Three-step first-run wizard, shown only when something is missing.
// Step 2 is a LIVE checklist: it re-polls the real permission state, no
// static text pretending things work. v2: glass card, step dots, switches.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Toggle } from "../../components/Toggle";
import { ipc, type Degradation, type Settings } from "../../lib/ipc";

export function Onboarding({
  settings,
  onSave,
  onClose,
}: {
  settings: Settings;
  onSave: (s: Settings) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [step, setStep] = useState(0);
  const [degradations, setDegradations] = useState<Degradation[]>([]);
  const [copied, setCopied] = useState(false);
  const [draft, setDraft] = useState(settings);

  useEffect(() => {
    const poll = () => ipc.getPermissionStatus().then(setDegradations);
    poll();
    const timer = window.setInterval(poll, 2500);
    return () => window.clearInterval(timer);
  }, []);

  const finish = () => {
    onSave({ ...draft, onboarding_done: true });
    onClose();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("onboarding.welcomeTitle")}
      className="fixed inset-0 z-10 flex items-center justify-center bg-bg/90 p-4 backdrop-blur-sm"
    >
      <div className="card w-full max-w-sm p-5">
        <div className="mb-4 flex justify-center gap-1.5" aria-hidden="true">
          {[0, 1, 2].map((i) => (
            <span
              key={i}
              className={`h-1.5 rounded-full transition-all ${
                i === step ? "w-6 bg-accent" : "w-1.5 bg-line"
              }`}
            />
          ))}
        </div>

        {step === 0 && (
          <div className="space-y-3">
            <h2 className="text-lg font-semibold text-accent">
              {t("onboarding.welcomeTitle")}
            </h2>
            <p className="text-sm">{t("onboarding.welcomeBody1")}</p>
            <p className="text-sm">{t("onboarding.welcomeBody2")}</p>
            <p className="text-sm text-ink-2">{t("onboarding.welcomeBody3")}</p>
          </div>
        )}

        {step === 1 && (
          <div className="space-y-3">
            <h2 className="text-lg font-semibold">
              {t("onboarding.permissionsTitle")}
            </h2>
            {degradations.length === 0 ? (
              <p
                className="rounded-xl border border-line bg-surface-2 p-3 text-sm text-ok"
                role="status"
              >
                {t("onboarding.allGood")}
              </p>
            ) : (
              <div className="space-y-2 text-sm" role="status">
                <p>{t("onboarding.missing")}</p>
                <ul className="space-y-2">
                  {degradations.map((d, i) => (
                    <li
                      key={i}
                      className="rounded-xl border border-line bg-surface-2 p-3"
                    >
                      <span className="text-alert">{d.detail}</span>
                      {d.help && (
                        <div className="mono mt-1 text-xs text-ink-2">
                          {d.help}
                        </div>
                      )}
                    </li>
                  ))}
                </ul>
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    className="min-h-8 rounded-full bg-accent px-4 py-1 font-semibold text-on-accent"
                    onClick={() =>
                      ipc
                        .requestPermission()
                        .then((granted) => {
                          if (!granted)
                            ipc.openPermissionSettings().catch(() => {});
                        })
                        .catch(() => {})
                    }
                  >
                    {t("degraded.grant")}
                  </button>
                  <button
                    type="button"
                    className="min-h-8 rounded-full border border-line px-3 py-1"
                    onClick={() => ipc.openPermissionSettings().catch(() => {})}
                  >
                    {t("onboarding.openSystemSettings")}
                  </button>
                  {degradations.some((d) => d.help?.includes("\n")) && (
                    <button
                      type="button"
                      className="min-h-8 rounded-full border border-line px-3 py-1"
                      onClick={() => {
                        const commands = degradations
                          .map((d) => d.help)
                          .filter(Boolean)
                          .join("\n");
                        navigator.clipboard.writeText(commands).then(() => {
                          setCopied(true);
                          window.setTimeout(() => setCopied(false), 2000);
                        });
                      }}
                    >
                      {copied
                        ? t("onboarding.copied")
                        : t("onboarding.copyCommands")}
                    </button>
                  )}
                </div>
                <p className="text-xs text-ink-2">{t("onboarding.recheck")}</p>
              </div>
            )}
          </div>
        )}

        {step === 2 && (
          <div className="space-y-1">
            <h2 className="mb-2 text-lg font-semibold">
              {t("onboarding.prefsTitle")}
            </h2>
            <div className="hairline-rows">
              <div className="flex min-h-8 items-center justify-between gap-3 py-2.5 text-sm">
                <span>{t("settings.autostart")}</span>
                <Toggle
                  checked={draft.autostart}
                  onChange={(v) => setDraft({ ...draft, autostart: v })}
                  label={t("settings.autostart")}
                />
              </div>
              <div className="py-2.5 text-sm">
                <div className="flex items-center justify-between">
                  <span>{t("settings.interval")}</span>
                  <span className="mono text-ink-2">
                    {draft.interval_secs}s
                  </span>
                </div>
                <input
                  type="range"
                  aria-label={t("settings.interval")}
                  min={10}
                  max={240}
                  step={5}
                  value={draft.interval_secs}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      interval_secs: Number(e.target.value),
                    })
                  }
                  className="mt-1 w-full"
                />
              </div>
              <div className="flex min-h-8 items-center justify-between gap-3 py-2.5 text-sm">
                <span>{t("settings.scheduleEnabled")}</span>
                <Toggle
                  checked={draft.schedule.enabled}
                  onChange={(v) =>
                    setDraft({
                      ...draft,
                      schedule: { ...draft.schedule, enabled: v },
                    })
                  }
                  label={t("settings.scheduleEnabled")}
                />
              </div>
            </div>
          </div>
        )}

        <div className="mt-5 flex items-center justify-between">
          <button
            type="button"
            className="min-h-8 px-2 py-1 text-sm text-ink-2 hover:text-ink"
            onClick={finish}
          >
            {t("onboarding.skip")}
          </button>
          <div className="flex gap-2">
            {step > 0 && (
              <button
                type="button"
                className="min-h-8 rounded-full border border-line px-3 py-1 text-sm"
                onClick={() => setStep(step - 1)}
              >
                {t("onboarding.back")}
              </button>
            )}
            <button
              type="button"
              className="min-h-8 rounded-full bg-accent px-4 py-1 text-sm font-semibold text-on-accent"
              onClick={() => (step < 2 ? setStep(step + 1) : finish())}
            >
              {step < 2 ? t("onboarding.next") : t("onboarding.done")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
