import type { CrossEnvSignal } from "@/lib/types";

interface Props {
  signal: CrossEnvSignal | null;
}

export function CrossEnvCallout({ signal }: Props) {
  if (!signal) return null;
  return (
    <div className="callout-cross-env">
      <p className="callout-cross-env-label">Predicted from staging</p>
      <p className="callout-cross-env-body">
        Seen on a non-production environment {signal.daysBeforeProd} day
        {signal.daysBeforeProd === 1 ? "" : "s"} ago - same issue now active on production.
      </p>
    </div>
  );
}
