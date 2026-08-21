import * as React from "react";
import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatRelativeTime } from "@/lib/format";
import { useCurrentTime } from "@/lib/useCurrentTime";

type MetaTone = "neutral" | "info" | "success" | "warning";

const TONE_CLASS: Record<MetaTone, string> = {
  neutral: "text-muted-foreground",
  info: "text-blue-300",
  success: "text-emerald-300",
  warning: "text-amber-300",
};

function SurfaceMetaBadge({
  children,
  tone = "neutral",
  className,
  title,
}: {
  children: React.ReactNode;
  tone?: MetaTone;
  className?: string;
  title?: string;
}) {
  return (
    <span
      title={title}
      className={cn("surface-meta-badge text-micro", TONE_CLASS[tone], className)}>
      {children}
    </span>
  );
}

export function FreshnessBadge({
  timestamp,
  loading = false,
  prefix = "Verified",
  emptyLabel = "Not verified yet",
  staleAfterMs,
  className,
}: {
  timestamp?: Date | null;
  loading?: boolean;
  prefix?: string;
  emptyLabel?: string;
  staleAfterMs?: number;
  className?: string;
}) {
  const nowMs = useCurrentTime();

  if (loading) {
    return (
      <SurfaceMetaBadge tone="warning" className={className}>
        <Loader2 className="icon-xs animate-spin" />
        Refreshing
      </SurfaceMetaBadge>
    );
  }

  if (!timestamp || Number.isNaN(timestamp.getTime())) {
    return (
      <SurfaceMetaBadge tone="warning" className={className}>
        {emptyLabel}
      </SurfaceMetaBadge>
    );
  }

  const ageMs = nowMs - timestamp.getTime();
  const tone: MetaTone = staleAfterMs != null && ageMs > staleAfterMs ? "warning" : "success";

  return (
    <SurfaceMetaBadge tone={tone} className={className} title={timestamp.toLocaleString()}>
      {prefix} {formatRelativeTime(timestamp, nowMs)}
    </SurfaceMetaBadge>
  );
}
