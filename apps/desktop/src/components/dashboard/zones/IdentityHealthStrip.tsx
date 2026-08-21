import { Bell } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatRelativeTime } from "@/lib/tokens";
import { useCurrentTime } from "@/lib/useCurrentTime";
import type { SiteVerdict } from "@/lib/dashboard/types";

interface StackChip {
  framework: string | null;
  host: string | null;
  environment: string | null;
}

interface Props {
  domain: string;
  stack: StackChip;
  sslDaysRemaining: number | null;
  verdict: SiteVerdict;
  lastScanIso: string | null;
  unreadAlertCount: number;
  onOpenAlerts: () => void;
}

function verdictDotClass(kind: SiteVerdict["kind"]) {
  if (kind === "healthy") return "bg-score-excellent health-dot--healthy";
  if (kind === "blocked") return "bg-severity-critical health-dot--blocked";
  return "bg-severity-high health-dot--warning";
}

function sslDaysColor(days: number) {
  if (days < 14) return "text-severity-critical ssl-days-warn";
  if (days < 30) return "text-severity-high ssl-days-warn";
  return "text-muted-foreground";
}

function formatSslCertificateLabel(days: number) {
  if (days < 0) {
    const overdueDays = Math.abs(days);
    return `SSL certificate expired ${overdueDays} day${overdueDays === 1 ? "" : "s"} ago`;
  }
  if (days === 0) return "SSL certificate expires today";
  return `SSL certificate expires in ${days} day${days === 1 ? "" : "s"}`;
}

export function IdentityHealthStrip({
  domain,
  stack,
  sslDaysRemaining,
  verdict,
  lastScanIso,
  unreadAlertCount,
  onOpenAlerts,
}: Props) {
  const nowMs = useCurrentTime();
  const stackParts = [stack.framework, stack.host, stack.environment].filter(Boolean);
  const hasStack = stackParts.length > 0;

  const timeAgo = lastScanIso ? formatRelativeTime(new Date(lastScanIso), nowMs) : null;
  const metaParts: string[] = [];
  if (timeAgo) metaParts.push(`last scan ${timeAgo}`);
  const meta = metaParts.join(" · ");

  return (
    <div className="card card--compact identity-strip">
      <span className={`identity-dot ${verdictDotClass(verdict.kind)}`} />

      <strong className="identity-domain">{domain}</strong>

      {hasStack && (
        <span className="text-meta identity-stack-chip ghost-border">{stackParts.join(" · ")}</span>
      )}

      {sslDaysRemaining !== null ? (
        <span className={`text-meta ${sslDaysColor(sslDaysRemaining)}`}>
          {formatSslCertificateLabel(sslDaysRemaining)}
        </span>
      ) : null}

      <span className="identity-strip-right">
        {meta && <span className="text-meta tabular-nums">{meta}</span>}
        {unreadAlertCount > 0 && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onOpenAlerts}
            aria-label={`Open alerts: ${unreadAlertCount} alert${unreadAlertCount === 1 ? "" : "s"}`}>
            <Bell className="icon-sm" aria-hidden="true" />
            {unreadAlertCount} alert{unreadAlertCount === 1 ? "" : "s"}
          </Button>
        )}
      </span>
    </div>
  );
}
