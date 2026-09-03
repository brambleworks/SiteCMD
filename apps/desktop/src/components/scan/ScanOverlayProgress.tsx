import { ProgressBar } from "@/components/ui/progress-bar";
import { useScanRunPercent } from "@/components/scan/useScanRunPercent";

const RING_RADIUS = 58;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

/**
 * The ring and its percent label. Only this subtree follows the 50 ms glide
 * clock; the rest of the overlay re-renders on whole-number changes.
 */
export function ScanOverlayRing({ color, ringClass }: { color: string; ringClass: string }) {
  const fraction = useScanRunPercent();
  return (
    <div className="scan-score-hero-shell">
      <div className={`scan-progress-ping scan-overlay-ping animate-ping ${ringClass}`} />
      <svg className="scan-overlay-ring-svg" viewBox="0 0 128 128">
        <circle
          cx="64"
          cy="64"
          r={RING_RADIUS}
          fill="none"
          stroke="currentColor"
          strokeOpacity={0.06}
          strokeWidth="2.5"
        />
        <circle
          cx="64"
          cy="64"
          r={RING_RADIUS}
          fill="none"
          stroke={color}
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeDasharray={`${RING_CIRCUMFERENCE}`}
          strokeDashoffset={`${RING_CIRCUMFERENCE * (1 - fraction / 100)}`}
          className="scan-overlay-ring-progress"
        />
      </svg>
      <div className="scan-overlay-ring-label">
        <div className="scan-overlay-pct" data-testid="scan-progress-percent">
          {Math.round(fraction)}
          <span className="scan-overlay-pct-unit text-muted-foreground">%</span>
        </div>
      </div>
    </div>
  );
}

/** The linear bar under the status row, on the same glide clock as the ring. */
export function ScanOverlayBar({ color }: { color: string }) {
  const fraction = useScanRunPercent();
  return (
    <ProgressBar
      percent={fraction}
      color={color}
      label="Scan progress"
      className="scan-overlay-bar-fill"
      trackClassName="scan-overlay-bar-track"
    />
  );
}
