// Signature animated coffee carafe for the Presence hero (a line-art pot,
// our own drawing). The coffee level IS the live idle progress: it fills
// toward the interval with a gently waving surface, and drains on a poke.
// Steam rises while active. Pure SVG + CSS, reduced-motion aware.

export function BrewingCup({
  active,
  fill,
  color,
}: {
  active: boolean;
  /** 0..1 coffee level (idle progress toward the interval). */
  fill: number;
  /** CSS color for the coffee / accents. */
  color: string;
}) {
  const level = Math.max(0, Math.min(1, fill));
  // Carafe bowl: circle centered (60,74), radius 38 → interior y 36..112.
  const top = 40;
  const bottom = 110;
  const surfaceY = bottom - level * (bottom - top);

  return (
    <svg viewBox="0 0 120 120" className="h-full w-full" aria-hidden="true">
      <defs>
        <clipPath id="carafe-bowl">
          <circle cx="60" cy="74" r="36" />
        </clipPath>
        <linearGradient id="coffee-grad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.95" />
          <stop offset="100%" stopColor={color} stopOpacity="0.6" />
        </linearGradient>
      </defs>

      {/* Steam */}
      {active && (
        <g stroke={color} strokeWidth="3" strokeLinecap="round" fill="none">
          <path
            className="steam"
            style={{ ["--drift" as string]: "-3px" }}
            d="M50 26 q-4 -6 0 -12 q4 -6 0 -12"
          />
          <path
            className="steam"
            style={{ ["--drift" as string]: "3px", animationDelay: "0.8s" }}
            d="M64 26 q4 -6 0 -12 q-4 -6 0 -12"
          />
        </g>
      )}

      {/* Coffee fill with a waving surface, clipped to the bowl */}
      <g clipPath="url(#carafe-bowl)">
        <path
          d={`M20 ${surfaceY}
              q 10 -5 20 0 t 20 0 t 20 0 t 20 0
              L 100 120 L 20 120 Z`}
          fill="url(#coffee-grad)"
          style={{ transition: "d 0.9s ease" }}
        >
          {active && (
            <animate
              attributeName="d"
              dur="3.2s"
              repeatCount="indefinite"
              values={`M20 ${surfaceY} q 10 -5 20 0 t 20 0 t 20 0 t 20 0 L100 120 L20 120 Z;
                       M20 ${surfaceY} q 10 5 20 0 t 20 0 t 20 0 t 20 0 L100 120 L20 120 Z;
                       M20 ${surfaceY} q 10 -5 20 0 t 20 0 t 20 0 t 20 0 L100 120 L20 120 Z`}
            />
          )}
        </path>
      </g>

      {/* Bowl outline */}
      <circle
        cx="60"
        cy="74"
        r="36"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="4"
      />
      {/* Neck + lid */}
      <path
        d="M40 42 L44 24 h32 l4 18"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="4"
        strokeLinejoin="round"
      />
      <path
        d="M40 42 h40"
        stroke={color}
        strokeWidth="4"
        strokeLinecap="round"
      />
      {/* Tubular handle on the right */}
      <path
        d="M88 42 h10 a8 8 0 0 1 8 8 v34"
        fill="none"
        stroke="var(--accent)"
        strokeWidth="5"
        strokeLinecap="round"
      />
      {/* Graduation ticks */}
      <g stroke="var(--ink-2)" strokeWidth="3.5" strokeLinecap="round">
        <path d="M74 64 h8" />
        <path d="M74 78 h8" />
        <path d="M74 92 h8" />
      </g>
    </svg>
  );
}
