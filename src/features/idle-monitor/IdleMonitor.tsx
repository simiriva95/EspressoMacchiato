// The signature element: the OS idle counter, live. A big tabular-mono
// number climbing second by second, a thin crema bar filling toward the
// poke interval, and a green flash when a poke resets it. Watch it once,
// understand it works, forget the app (spec §7).

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatClock, formatSeconds } from "../../lib/format";
import { ipc, type PokeReport, type StatusSnapshot } from "../../lib/ipc";

interface Props {
  status: StatusSnapshot | null;
  /** Timestamp of the last successful poke; a change triggers the flash. */
  pokeSignal?: number;
  compact?: boolean;
}

export function IdleMonitor({ status, pokeSignal = 0, compact = false }: Props) {
  const { t } = useTranslation();
  const [idle, setIdle] = useState<number | null>(null);
  const [idleError, setIdleError] = useState<string | null>(null);
  const [testReport, setTestReport] = useState<PokeReport | null>(null);
  const [testing, setTesting] = useState(false);
  const [flashing, setFlashing] = useState(false);

  useEffect(() => {
    const poll = async () => {
      try {
        setIdle(await ipc.getIdleSeconds());
        setIdleError(null);
      } catch (e) {
        setIdleError(String(e));
      }
    };
    poll();
    const timer = window.setInterval(poll, 1000);
    return () => window.clearInterval(timer);
  }, []);

  // One orchestrated animation: number back to zero with a quick ok-green
  // flash. prefers-reduced-motion turns it into a static color (CSS).
  useEffect(() => {
    if (pokeSignal === 0) return;
    const start = window.setTimeout(() => {
      setIdle(0);
      setFlashing(true);
    }, 0);
    const stop = window.setTimeout(() => setFlashing(false), 700);
    return () => {
      window.clearTimeout(start);
      window.clearTimeout(stop);
    };
  }, [pokeSignal]);

  const testNow = useCallback(async () => {
    setTesting(true);
    try {
      setTestReport(await ipc.pokeNow());
    } finally {
      setTesting(false);
    }
  }, []);

  const interval = status?.interval_secs ?? 60;
  const fillPercent =
    idle !== null ? Math.min(100, (idle / interval) * 100) : 0;

  return (
    <section
      aria-label={t("a11y.idleMonitorRegion")}
      className="card p-4"
    >
      <h2 className="text-xs font-semibold uppercase tracking-wide text-ink-2">
        {t("idle.title")}
      </h2>

      <div className="mt-2">
        <div
          className={`mono ${compact ? "text-xl" : "text-2xl"} ${
            flashing ? "poke-flash" : ""
          }`}
          data-testid="idle-value"
        >
          {idleError !== null
            ? t("common.dash")
            : idle !== null
              ? formatSeconds(idle)
              : t("common.dash")}
        </div>
        {/* Decorative: mirrors the number against the interval. */}
        <div aria-hidden="true" className="mt-2 h-1 w-full rounded bg-line">
          <div
            className="h-1 rounded bg-accent"
            style={{ width: `${fillPercent}%` }}
          />
        </div>
        <div className="mt-1 text-xs text-ink-2">
          {t("idle.sourcePrefix")}{" "}
          <span data-testid="idle-source">
            {status?.idle_source ?? t("common.dash")}
          </span>
        </div>
        {idleError !== null && (
          <p className="mt-1 text-xs text-alert" role="status">
            {t("idle.notReadable", { error: idleError })}
          </p>
        )}
      </div>

      {!compact && (
        <dl className="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
          <dt className="text-ink-2">{t("idle.pokes")}</dt>
          <dd className="mono" data-testid="poke-count">
            {status?.poke_count ?? 0}
          </dd>
          <dt className="text-ink-2">{t("idle.lastPoke")}</dt>
          <dd className="mono">
            {formatClock(status?.last_poke_unix_ms ?? null)}
          </dd>
          <dt className="text-ink-2">{t("idle.lastResult")}</dt>
          <dd data-testid="last-result">
            {status?.last_poke_ok === null || status?.last_poke_ok === undefined
              ? t("common.dash")
              : status.last_poke_ok
                ? t("idle.ok")
                : (status.last_poke_error ?? t("common.dash"))}
          </dd>
          <dt className="text-ink-2">{t("idle.nextPoke")}</dt>
          <dd className="mono" data-testid="next-poke">
            {status?.next_poke_in_secs != null
              ? formatSeconds(status.next_poke_in_secs)
              : t("common.dash")}
          </dd>
        </dl>
      )}

      <div className="mt-3 space-y-1">
        <button
          type="button"
          onClick={testNow}
          disabled={testing}
          className="min-h-8 rounded-full border border-line px-4 py-1.5 text-sm hover:bg-bg disabled:opacity-50"
        >
          {t("idle.testNow")}
        </button>
        {testReport && (
          <p className="text-xs text-ink-2" data-testid="test-result" role="status">
            {testReport.ok
              ? t("idle.testResult", {
                  before: testReport.idle_before?.toFixed(1) ?? "?",
                  after: testReport.idle_after?.toFixed(1) ?? "?",
                })
              : t("idle.testFailed", {
                  error: testReport.error ?? t("common.dash"),
                })}
          </p>
        )}
      </div>
    </section>
  );
}
