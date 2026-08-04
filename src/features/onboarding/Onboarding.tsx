// Three-step first-run wizard, shown only when something is missing.
// Step 2 is a LIVE checklist: it re-polls the real permission state, no
// static text pretending things work.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
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
      className="fixed inset-0 z-10 flex items-center justify-center bg-bg/95 p-4"
    >
      <div className="w-full max-w-sm rounded-lg border border-line bg-surface p-5">
        {step === 0 && (
          <div className="space-y-3">
            <h2 className="display text-xl text-accent">
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
              <p className="text-sm text-ok" role="status">
                {t("onboarding.allGood")}
              </p>
            ) : (
              <div className="space-y-2 text-sm" role="status">
                <p>{t("onboarding.missing")}</p>
                <ul className="list-disc space-y-2 pl-4">
                  {degradations.map((d, i) => (
                    <li key={i}>
                      {d.detail}
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
                    className="min-h-8 rounded border border-line px-2 py-1"
                    onClick={() => ipc.openPermissionSettings().catch(() => {})}
                  >
                    {t("onboarding.openSystemSettings")}
                  </button>
                  {degradations.some((d) => d.help?.includes("\n")) && (
                    <button
                      type="button"
                      className="min-h-8 rounded border border-line px-2 py-1"
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
          <div className="space-y-3">
            <h2 className="text-lg font-semibold">
              {t("onboarding.prefsTitle")}
            </h2>
            <label className="flex min-h-8 items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={draft.autostart}
                onChange={(e) =>
                  setDraft({ ...draft, autostart: e.target.checked })
                }
              />
              {t("settings.autostart")}
            </label>
            <label className="block text-sm">
              {t("settings.interval")}:{" "}
              <span className="mono">{draft.interval_secs}s</span>
              <input
                type="range"
                min={10}
                max={240}
                step={5}
                value={draft.interval_secs}
                onChange={(e) =>
                  setDraft({ ...draft, interval_secs: Number(e.target.value) })
                }
                className="w-full"
              />
            </label>
            <label className="flex min-h-8 items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={draft.schedule.enabled}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    schedule: { ...draft.schedule, enabled: e.target.checked },
                  })
                }
              />
              {t("settings.scheduleEnabled")}
            </label>
          </div>
        )}

        <div className="mt-5 flex items-center justify-between">
          <button
            type="button"
            className="min-h-8 px-2 py-1 text-sm text-ink-2"
            onClick={finish}
          >
            {t("onboarding.skip")}
          </button>
          <div className="flex gap-2">
            {step > 0 && (
              <button
                type="button"
                className="min-h-8 rounded border border-line px-3 py-1 text-sm"
                onClick={() => setStep(step - 1)}
              >
                {t("onboarding.back")}
              </button>
            )}
            {step < 2 ? (
              <button
                type="button"
                className="min-h-8 rounded bg-accent px-3 py-1 text-sm font-semibold text-on-accent"
                onClick={() => setStep(step + 1)}
              >
                {t("onboarding.next")}
              </button>
            ) : (
              <button
                type="button"
                className="min-h-8 rounded bg-accent px-3 py-1 text-sm font-semibold text-on-accent"
                onClick={finish}
              >
                {t("onboarding.done")}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
