import type { Evidence } from "@/lib/types";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface Props {
  evidence: Evidence[];
}

export function EvidenceDisclosure({ evidence }: Props) {
  const nowMs = useCurrentTime();

  if (evidence.length === 0) return null;
  return (
    <details className="evidence-disclosure">
      <summary className="evidence-summary">Show evidence ({evidence.length})</summary>
      <ul className="ev-list">
        {evidence.map((e, i) => (
          <li key={i} className="ev-row">
            <span className="ev-kind">{e.kind}</span>
            <span className="ev-detail">{e.detail}</span>
            {e.timestamp ? (
              <span className="ev-timestamp">{formatRelativeTime(e.timestamp, nowMs)}</span>
            ) : null}
          </li>
        ))}
      </ul>
    </details>
  );
}
