// The signature diagnostic panel: the OS idle counter, live. Watching it
// climb and reset to zero is what makes the app credible. Raw version (M1);
// design tokens and the steam animation land in M3.

import { useCallback, useEffect, useRef, useState } from "react";
import { ipc, type PokeReport, type StatusSnapshot } from "../../lib/ipc";
import { formatClock, formatSeconds } from "../../lib/format";

interface Props {
  status: StatusSnapshot | null;
}

export function IdleMonitor({ status }: Props) {
  const [idle, setIdle] = useState<number | null>(null);
  const [idleError, setIdleError] = useState<string | null>(null);
  const [testReport, setTestReport] = useState<PokeReport | null>(null);
  const [testing, setTesting] = useState(false);
  const pollRef = useRef<number | null>(null);

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
    pollRef.current = window.setInterval(poll, 1000);
    return () => {
      if (pollRef.current !== null) window.clearInterval(pollRef.current);
    };
  }, []);

  const testNow = useCallback(async () => {
    setTesting(true);
    try {
      setTestReport(await ipc.pokeNow());
    } finally {
      setTesting(false);
    }
  }, []);

  return (
    <section aria-label="Idle monitor" className="rounded-lg border border-gray-300 p-4 space-y-3">
      <h2 className="text-sm font-semibold uppercase tracking-wide">
        Idle monitor
      </h2>

      <div>
        <div className="mono text-4xl" aria-live="off" data-testid="idle-value">
          {idleError !== null
            ? "n/a"
            : idle !== null
              ? formatSeconds(idle)
              : "…"}
        </div>
        <div className="text-xs text-gray-500">
          system idle time · source:{" "}
          <span data-testid="idle-source">{status?.idle_source ?? "…"}</span>
        </div>
        {idleError !== null && (
          <div className="text-xs text-red-700" role="status">
            Idle time not readable on this system: {idleError}
          </div>
        )}
      </div>

      <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
        <dt className="text-gray-500">Pokes this session</dt>
        <dd className="mono" data-testid="poke-count">
          {status?.poke_count ?? 0}
        </dd>
        <dt className="text-gray-500">Last poke</dt>
        <dd className="mono">{formatClock(status?.last_poke_unix_ms ?? null)}</dd>
        <dt className="text-gray-500">Last result</dt>
        <dd data-testid="last-result">
          {status?.last_poke_ok === null || status?.last_poke_ok === undefined
            ? "—"
            : status.last_poke_ok
              ? "ok"
              : (status.last_poke_error ?? "failed")}
        </dd>
        <dt className="text-gray-500">Next poke in</dt>
        <dd className="mono" data-testid="next-poke">
          {status?.next_poke_in_secs != null
            ? formatSeconds(status.next_poke_in_secs)
            : "—"}
        </dd>
      </dl>

      <div className="space-y-1">
        <button
          type="button"
          onClick={testNow}
          disabled={testing}
          className="rounded border border-gray-400 px-3 py-1.5 text-sm hover:bg-gray-100 disabled:opacity-50"
        >
          Test now
        </button>
        {testReport && (
          <p className="text-xs" data-testid="test-result" role="status">
            {testReport.ok
              ? `Poke sent. Idle before: ${testReport.idle_before?.toFixed(1) ?? "?"}s → after: ${testReport.idle_after?.toFixed(1) ?? "?"}s`
              : (testReport.error ?? "Poke failed")}
          </p>
        )}
      </div>
    </section>
  );
}
