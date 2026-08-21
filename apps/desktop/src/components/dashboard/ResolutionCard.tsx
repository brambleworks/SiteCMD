import type { ReactNode } from "react";

interface Props {
  // A deploy-to-resolution correlation returned by get_project_correlations.
  correlation: {
    correlationType: string; // "deploy_to_resolution"
    description: string; // human-readable summary
    sourceTimestamp: string;
    confidence: string;
  };
}

function formatTime(iso: string): string {
  const date = new Date(iso);
  if (!Number.isFinite(date.getTime())) return iso;
  return date.toLocaleString();
}

export function ResolutionCard({ correlation }: Props): ReactNode {
  if (correlation.correlationType !== "deploy_to_resolution") return null;
  return (
    <div className="resolution-card">
      <p className="resolution-card-title">Resolved</p>
      <p className="resolution-card-body">{correlation.description}</p>
      <p className="resolution-card-meta">{formatTime(correlation.sourceTimestamp)}</p>
    </div>
  );
}
