// Minimal stroked icons drawn inline — no icon library dependency. 20px
// grid, currentColor stroke.

type P = { className?: string };

const base = "h-5 w-5";

export function CupIcon({ className }: P) {
  return (
    <svg viewBox="0 0 20 20" fill="none" className={className ?? base} aria-hidden="true">
      <path
        d="M4 6h9v5a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V6Z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
      <path
        d="M13 7h1.5a2 2 0 1 1 0 4H13"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <path
        d="M6.5 2.5c-.5.7-.5 1.3 0 2M9.5 2.5c-.5.7-.5 1.3 0 2"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function BoltIcon({ className }: P) {
  return (
    <svg viewBox="0 0 20 20" fill="none" className={className ?? base} aria-hidden="true">
      <path
        d="M11 2 4 11h5l-1 7 7-9h-5l1-7Z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function ChartIcon({ className }: P) {
  return (
    <svg viewBox="0 0 20 20" fill="none" className={className ?? base} aria-hidden="true">
      <path d="M3 17h14" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <rect x="4" y="9" width="3" height="5" rx="1" stroke="currentColor" strokeWidth="1.5" />
      <rect x="9" y="5" width="3" height="9" rx="1" stroke="currentColor" strokeWidth="1.5" />
      <rect x="14" y="11" width="3" height="3" rx="1" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function SlidersIcon({ className }: P) {
  return (
    <svg viewBox="0 0 20 20" fill="none" className={className ?? base} aria-hidden="true">
      <path d="M4 6h8M15 6h1M4 14h1M8 14h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="13" cy="6" r="2" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="6" cy="14" r="2" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}
