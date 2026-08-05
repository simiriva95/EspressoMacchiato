// Duration options for activation. "Until HH:MM" is converted to seconds
// client-side (same machine, same clock as the backend).

export type DurationChoice =
  | { kind: "indefinite" }
  | { kind: "minutes"; minutes: number }
  | { kind: "until"; time: string } // "HH:MM"
  | { kind: "epoch"; endMs: number }; // absolute end (e.g. meeting end)

/** Seconds from `now` until HH:MM today, or tomorrow if already past. */
export function secondsUntil(hhmm: string, now: Date = new Date()): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(hhmm.trim());
  if (!m) return null;
  const hours = Number(m[1]);
  const minutes = Number(m[2]);
  if (hours > 23 || minutes > 59) return null;
  const nowSecs =
    now.getHours() * 3600 + now.getMinutes() * 60 + now.getSeconds();
  const targetSecs = hours * 3600 + minutes * 60;
  const delta = targetSecs - nowSecs;
  return delta > 0 ? delta : delta + 86_400;
}

export function toDurationSecs(
  choice: DurationChoice,
  now: Date = new Date(),
): number | undefined {
  switch (choice.kind) {
    case "indefinite":
      return undefined;
    case "minutes":
      return choice.minutes * 60;
    case "until":
      return secondsUntil(choice.time, now) ?? undefined;
    case "epoch":
      return Math.max(60, Math.round((choice.endMs - now.getTime()) / 1000));
  }
}
