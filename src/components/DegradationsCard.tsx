// Degraded mode is declared, never silent: what's missing, why it matters,
// and the exact remediation.

import { useTranslation } from "react-i18next";
import { ipc, type Degradation } from "../lib/ipc";

export function DegradationsCard({
  degradations,
}: {
  degradations: Degradation[];
}) {
  const { t } = useTranslation();
  if (degradations.length === 0) return null;
  return (
    <div className="card border-alert p-3 text-sm">
      <p className="font-semibold text-alert">{t("degraded.title")}</p>
      <ul className="mt-1 list-disc space-y-1 pl-4">
        {degradations.map((d, i) => (
          <li key={i}>
            {d.detail}
            {d.help && (
              <div className="mono mt-1 text-xs text-ink-2">{d.help}</div>
            )}
          </li>
        ))}
      </ul>
      <button
        type="button"
        className="mt-2 min-h-8 rounded-full border border-line px-3 py-1"
        onClick={() => ipc.openPermissionSettings().catch(() => {})}
      >
        {t("degraded.openSystemSettings")}
      </button>
    </div>
  );
}
