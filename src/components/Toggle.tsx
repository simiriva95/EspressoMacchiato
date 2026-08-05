// Refined switch: slim track, floating knob with depth, spring-ish easing.
// Disabled/reduced-motion handled by the global reset.

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
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 disabled:opacity-40",
        checked
          ? "bg-accent shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06)]"
          : "bg-line",
      )}
      style={{ transitionTimingFunction: "cubic-bezier(0.34, 1.56, 0.64, 1)" }}
    >
      <span
        aria-hidden="true"
        className={clsx(
          "inline-block h-5 w-5 rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.3)] transition-transform duration-200",
          checked ? "translate-x-[22px]" : "translate-x-0.5",
        )}
        style={{
          transitionTimingFunction: "cubic-bezier(0.34, 1.56, 0.64, 1)",
        }}
      />
    </button>
  );
}
