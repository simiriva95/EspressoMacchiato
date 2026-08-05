// The signature animated cup for the Presence hero: a moka-style cup with
// three rising steam wisps when active, and a coffee level that fills the
// cup toward the poke interval. Pure SVG + CSS, reduced-motion aware.

export function BrewingCup({
  active,
  fill,
  color,
}: {
  active: boolean;
  /** 0..1 coffee level (idle progress toward the interval). */
  fill: number;
  /** CSS color for accents/steam. */
  color: string;
}) {
  const level = Math.max(0, Math.min(1, fill));
  // Cup interior spans y 30..58; coffee surface rises as level grows.
  const surfaceY = 58 - level * 26;

  return (
    <svg viewBox="0 0 120 120" className="h-full w-full" aria-hidden="true">
      <defs>
        <clipPath id="cup-interior">
          <path d="M34 30 h44 l-4 30 a18 18 0 0 1-36 0 Z" />
        </clipPath>
        <linearGradient id="coffee" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.9" />
          <stop offset="100%" stopColor={color} stopOpacity="0.55" />
        </linearGradient>
      </defs>

      {/* Steam */}
      {active && (
        <g stroke={color} strokeWidth="3" strokeLinecap="round" fill="none">
          <path
            className="steam"
            style={{ ["--drift" as string]: "-3px", animationDelay: "0s" }}
            d="M48 24 q-4 -6 0 -12 q4 -6 0 -12"
          />
          <path
            className="steam"
            style={{ ["--drift" as string]: "2px", animationDelay: "0.6s" }}
            d="M60 24 q4 -6 0 -12 q-4 -6 0 -12"
          />
          <path
            className="steam"
            style={{ ["--drift" as string]: "4px", animationDelay: "1.2s" }}
            d="M72 24 q-4 -6 0 -12 q4 -6 0 -12"
          />
        </g>
      )}

      {/* Coffee fill (clipped to interior) */}
      <g clipPath="url(#cup-interior)">
        <rect
          x="30"
          y={surfaceY}
          width="60"
          height="40"
          fill="url(#coffee)"
          style={{ transition: "y 0.9s ease" }}
        />
      </g>

      {/* Cup outline */}
      <path
        d="M34 30 h44 l-4 30 a18 18 0 0 1-36 0 Z"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="3.5"
        strokeLinejoin="round"
      />
      {/* Handle */}
      <path
        d="M78 34 a12 12 0 0 1 0 22"
        fill="none"
        stroke="var(--ink)"
        strokeWidth="3.5"
        strokeLinecap="round"
      />
      {/* Saucer */}
      <path
        d="M30 88 h60"
        stroke="var(--ink)"
        strokeWidth="3.5"
        strokeLinecap="round"
      />
    </svg>
  );
}
