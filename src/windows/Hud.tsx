// Floating always-on-top pill: state dot + countdown (or idle counter).
// Drag anywhere; double-click toggles the engine.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatSeconds } from "../lib/format";
import { ipc } from "../lib/ipc";
import { useEngine } from "../lib/useEngine";

export function Hud() {
  const { t } = useTranslation();
  const { status } = useEngine();
  const [idle, setIdle] = useState<number | null>(null);

  useEffect(() => {
    const poll = () => ipc.getIdleSeconds().then(setIdle, () => setIdle(null));
    poll();
    const timer = window.setInterval(poll, 1000);
    return () => window.clearInterval(timer);
  }, []);

  const on = status !== null && status.state !== "off";
  const dot =
    status?.state === "active"
      ? "bg-ok"
      : status?.state === "degraded"
        ? "bg-alert"
        : status?.state === "suspended"
          ? "bg-accent"
          : "bg-ink-2";

  return (
    <button
      type="button"
      data-tauri-drag-region
      onDoubleClick={() => ipc.toggle()}
      aria-label={t("a11y.stateAnnouncement", {
        state: status ? t(`state.${status.state}`) : t("common.dash"),
      })}
      className="card flex h-screen w-screen cursor-default items-center gap-3 px-4 text-left"
    >
      <span
        aria-hidden="true"
        data-tauri-drag-region
        className={`h-2.5 w-2.5 shrink-0 rounded-full ${dot}`}
      />
      <span className="mono text-lg" data-testid="idle-value" data-tauri-drag-region>
        {on
          ? status?.remaining_secs != null
            ? formatSeconds(status.remaining_secs)
            : "∞"
          : idle !== null
            ? formatSeconds(idle)
            : t("common.dash")}
      </span>
      <span
        className="ml-auto truncate text-[10px] uppercase tracking-widest text-ink-2"
        data-tauri-drag-region
      >
        {status ? t(`state.${status.state}`) : ""}
      </span>
    </button>
  );
}
