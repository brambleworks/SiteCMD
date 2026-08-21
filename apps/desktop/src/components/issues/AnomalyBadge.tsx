interface Props {
  score: number | null;
}

export function AnomalyBadge({ score }: Props) {
  if (score == null) return null;
  const severity = Math.abs(score) > 5 ? "critical" : "warning";
  return (
    <span className="anomaly-badge" data-sev={severity}>
      Anomaly: {score.toFixed(1)}σ from baseline
    </span>
  );
}
