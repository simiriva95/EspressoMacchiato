// One settings row: label (+ optional description) left, control right.
// Rows stack inside a .card with hairline dividers.

import type { ReactNode } from "react";

export function Row({
  label,
  description,
  children,
  stacked = false,
}: {
  label: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  /** Control below the label instead of beside it (wide controls). */
  stacked?: boolean;
}) {
  return (
    <div className="py-3 first:pt-0 last:pb-0">
      <div
        className={
          stacked
            ? "space-y-2"
            : "flex min-h-8 items-center justify-between gap-4"
        }
      >
        <div className="min-w-0">
          <div className="text-sm">{label}</div>
          {description && (
            <div className="mt-0.5 text-xs text-ink-2">{description}</div>
          )}
        </div>
        <div className={stacked ? "" : "flex shrink-0 items-center gap-2"}>
          {children}
        </div>
      </div>
    </div>
  );
}
