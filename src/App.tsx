import { useEffect, useState } from "react";
import { IdleMonitor } from "./features/idle-monitor/IdleMonitor";
import {
  ipc,
  type ActivityStrategy,
  type StatusSnapshot,
} from "./lib/ipc";

const STRATEGY_LABELS: Record<ActivityStrategy, string> = {
  zero_mouse_move: "Zero-delta mouse move (invisible)",
  harmless_key_tap: "Harmless key tap (F15)",
  nudge_and_return: "Nudge 1px and return (visible)",
};

function App() {
  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [interval, setIntervalSecs] = useState(60);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.getStatus().then((s) => {
      setStatus(s);
      setIntervalSecs(s.interval_secs);
    });
    ipc
      .onEngineEvent((e) => {
        if (e.type === "state_changed") {
          setStatus(e.status);
          setIntervalSecs(e.status.interval_secs);
        } else {
          // poke event: refresh counters
          ipc.getStatus().then(setStatus);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  const on = status !== null && status.state !== "off";

  return (
    <main className="mx-auto max-w-md space-y-4 p-4">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-bold">EspressoMacchiato</h1>
          <p className="text-sm text-gray-500" data-testid="state">
            {status?.state ?? "…"}
            {status?.state === "degraded" && ` — ${status.state_detail}`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => ipc.setActive(!on)}
          className={`rounded px-4 py-2 font-semibold text-white ${
            on ? "bg-red-800" : "bg-green-700"
          }`}
        >
          {on ? "Deactivate" : "Activate"}
        </button>
      </header>

      {status && status.degradations.length > 0 && (
        <div className="rounded border border-amber-500 bg-amber-50 p-3 text-sm">
          <p className="font-semibold">Reduced functionality</p>
          <ul className="list-disc pl-4">
            {status.degradations.map((d, i) => (
              <li key={i}>
                {d.detail}
                {d.help && <div className="mono text-xs">{d.help}</div>}
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="mt-2 rounded border border-amber-600 px-2 py-1"
            onClick={() => ipc.openPermissionSettings().catch(() => {})}
          >
            Open system settings
          </button>
        </div>
      )}

      <IdleMonitor status={status} />

      <section className="space-y-3 rounded-lg border border-gray-300 p-4">
        <h2 className="text-sm font-semibold uppercase tracking-wide">
          Settings
        </h2>

        <label className="block text-sm">
          Poke interval: <span className="mono">{interval}s</span>
          <input
            type="range"
            min={10}
            max={240}
            step={5}
            value={interval}
            onChange={(e) => setIntervalSecs(Number(e.target.value))}
            onMouseUp={() => ipc.setIntervalSecs(interval)}
            onTouchEnd={() => ipc.setIntervalSecs(interval)}
            onKeyUp={() => ipc.setIntervalSecs(interval)}
            className="w-full"
          />
          <span className="text-xs text-gray-500">
            Presence clients typically go Away after ~5 minutes of idle; keep
            this well below that.
          </span>
        </label>

        <label className="block text-sm">
          Strategy
          <select
            value={status?.strategy ?? "zero_mouse_move"}
            onChange={(e) =>
              ipc.setStrategy(e.target.value as ActivityStrategy)
            }
            className="mt-1 w-full rounded border border-gray-300 p-1"
          >
            {(status?.available_strategies ?? []).map((s) => (
              <option key={s} value={s}>
                {STRATEGY_LABELS[s]}
              </option>
            ))}
          </select>
        </label>

        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={status?.pause_when_input_recent ?? true}
            onChange={(e) => ipc.setPauseWhenInputRecent(e.target.checked)}
          />
          Skip pokes while I'm actually using the machine
        </label>
      </section>
    </main>
  );
}

export default App;
