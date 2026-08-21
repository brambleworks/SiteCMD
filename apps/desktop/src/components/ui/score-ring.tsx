import { getScoreCssVar } from "@/lib/tokens";

interface ScoreRingProps {
  value: number | null;
  total?: number;
  /** Ring fill percentage; defaults to `value / total * 100`. */
  percent?: number;
  /** CSS variable used for the ring and value, defaulting to the score band. */
  toneVar?: string;
  /** How the score label should be presented. Defaults to fraction style. */
  labelMode?: "fraction" | "percent" | "value" | "none";
  size?: number;
  strokeWidth?: number;
}

export function ScoreRing({
  value,
  total = 100,
  percent,
  toneVar,
  labelMode = "fraction",
  size = 96,
  strokeWidth,
}: ScoreRingProps) {
  const empty = value == null;
  const resolvedPercent =
    percent ?? (empty ? 0 : Math.max(0, Math.min(100, (value / total) * 100)));
  // Accept bare and pre-wrapped CSS variable names.
  const rawTone = toneVar ?? (empty ? "--muted-foreground" : getScoreCssVar(resolvedPercent));
  const color = rawTone.startsWith("var(") ? rawTone : `var(${rawTone})`;
  const sw = strokeWidth ?? Math.max(4, Math.round(size * 0.065));
  const radius = size / 2 - sw;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference - (circumference * resolvedPercent) / 100;
  const cx = size / 2;
  const cy = size / 2;
  const showValue = labelMode !== "none";
  const showDenominator = labelMode === "fraction" || labelMode === "percent";

  return (
    <div
      className="score-ring"
      style={{ width: size, height: size }}
      aria-hidden={empty ? true : undefined}>
      <svg className="score-ring-svg" width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle
          cx={cx}
          cy={cy}
          r={radius}
          fill="transparent"
          stroke="var(--muted)"
          strokeWidth={sw}
        />
        {!empty && (
          <circle
            cx={cx}
            cy={cy}
            r={radius}
            fill="transparent"
            stroke={color}
            strokeWidth={sw}
            strokeDasharray={circumference}
            strokeDashoffset={dashOffset}
            strokeLinecap="round"
          />
        )}
      </svg>
      <div className="score-ring-center">
        {showValue && !empty ? (
          <span
            className="score-ring-value"
            style={{
              color,
              fontSize: Math.round(size * 0.36),
            }}>
            {value}
          </span>
        ) : null}
        {showDenominator && !empty ? (
          <span
            className="text-muted-foreground score-ring-denominator"
            style={{ fontSize: Math.round(size * 0.115) }}>
            /{total}
          </span>
        ) : null}
      </div>
    </div>
  );
}
