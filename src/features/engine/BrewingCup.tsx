// Signature animated cup for the Presence hero (our own line-art drawing).
// Bold steam sweeps up while active; the coffee level IS the live idle
// progress — it fills toward the interval and drains on a poke. Pure SVG +
// CSS, reduced-motion aware.

export function BrewingCup({
  active,
  fill,
  color,
}: {
  active: boolean;
  /** 0..1 coffee level (idle progress toward the interval). */
  fill: number;
  /** CSS color for the coffee / steam / accents. */
  color: string;
}) {
  const level = Math.max(0, Math.min(1, fill));
  // Cup interior spans y 52..92; coffee surface rises with the level.
  const surfaceY = 92 - level * 40;

  return (
    <svg viewBox="0 0 120 120" className="h-full w-full" aria-hidden="true">
      <defs>
        <clipPath id="cup-interior">
          <path d="M34 50 h52 l-5 40 a21 21 0 0 1-42 0 Z" />
        </clipPath>
        <linearGradient id="coffee-grad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.95" />
          <stop offset="100%" stopColor={color} stopOpacity="0.6" />
        </linearGradient>
      </defs>

      {/* Bold steam: three thick wisps on an S-curve */}
      {active && (
        <g
          stroke={color}
          strokeWidth="6"
          strokeLinecap="round"
          fill="none"
          opacity="0.9"
        >
          <path
            className="steam"
            style={{ ["--drift" as string]: "-4px" }}
            d="M44 44 q-9 -9 0 -18 q9 -9 0 -18"
          />
          <path
            className="steam"
            style={{ ["--drift" as string]: "3px", animationDelay: "0.7s" }}
            d="M60 44 q9 -9 0 -18 q-9 -9 0 -18"
          />
          <path
            className="steam"
            style={{ ["--drift" as string]: "5px", animationDelay: "1.4s" }}
            d="M76 44 q-9 -9 0 -18 q9 -9 0 -18"
          />
        </g>
      )}

      {/* Coffee fill with a waving surface, clipped to the interior */}
      <g clipPath="url(#cup-interior)">
        <path
          d={`M28 ${surfaceY} q 8 -5 16 0 t 16 0 t 16 0 t 16 0 L92 120 L28 120 Z`}
          fill="url(#coffee-grad)"
          style={{ transition: "d 0.9s ease" }}
        >
          {active && (
            <animate
              attributeName="d"
              dur="3s"
              repeatCount="indefinite"
              values={`M28 ${surfaceY} q 8 -5 16 0 t 16 0 t 16 0 t 16 0 L92 120 L28 120 Z;
                       M28 ${surfaceY} q 8 5 16 0 t 16 0 t 16 0 t 16 0 L92 120 L28 120 Z;
                       M28 ${surfaceY} q 8 -5 16 0 t 16 0 t 16 0 t 16 0 L92 120 L28 120 Z`}
            />
          )}
        </path>
      </g>

      {/* Cup outline */}
      <path
        d="M34 50 h52 l-5 40 a21 21 0 0 1-42 0 Z"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="4"
        strokeLinejoin="round"
      />
      {/* Handle */}
      <path
        d="M86 54 a14 14 0 0 1 0 26"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="4"
        strokeLinecap="round"
      />
      {/* Saucer */}
      <path
        d="M28 100 h64"
        stroke="var(--ink)"
        strokeWidth="4"
        strokeLinecap="round"
      />
    </svg>
  );
}
