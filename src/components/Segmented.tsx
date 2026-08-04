// Segmented control (Auto | Dark | Light style).

import { clsx } from "clsx";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className="flex shrink-0 rounded-full border border-line bg-bg p-0.5"
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={value === option.value}
          onClick={() => onChange(option.value)}
          className={clsx(
            "min-h-7 rounded-full px-3 text-sm transition-colors",
            value === option.value
              ? "bg-accent font-semibold text-on-accent"
              : "text-ink-2 hover:text-ink",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
