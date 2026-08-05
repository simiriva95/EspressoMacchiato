// iOS-style switch. Pure CSS transition (≤150ms), disabled under
// prefers-reduced-motion by the global rule.

import { clsx } from "clsx";

export function Toggle({
  checked,
  onChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  /** Accessible name; the visible label lives in the surrounding Row. */
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={clsx(
        "relative h-7 w-12 shrink-0 rounded-full border transition-colors disabled:opacity-40",
        checked ? "border-accent bg-accent" : "border-line bg-bg",
      )}
    >
      <span
        aria-hidden="true"
        className={clsx(
          "absolute left-0 top-0.5 h-[22px] w-[22px] rounded-full bg-white shadow transition-transform",
          checked ? "translate-x-[24px]" : "translate-x-0.5",
        )}
      />
    </button>
  );
}
