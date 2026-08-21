import type { CrossProjectPattern } from "@/lib/types";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface Props {
  pattern: CrossProjectPattern | null;
}

export function CrossProjectLine({ pattern }: Props) {
  const nowMs = useCurrentTime();

  if (!pattern || pattern.projectCount === 0) return null;
  return (
    <p className="cross-project-line">
      You have hit this in {pattern.projectCount} other project
      {pattern.projectCount === 1 ? "" : "s"} (last seen{" "}
      {formatRelativeTime(pattern.lastSeenAt, nowMs)}).
    </p>
  );
}
