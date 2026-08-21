import { type ReactNode } from "react";
import { CheckCheck, ChevronRight, ExternalLink, RotateCcw, XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DossierNumberedSection,
  DossierRail,
  IssueDossierPanel,
} from "@/components/issues/IssueDossierPanel";
import type { AlertRow } from "@/lib/types";
import { openUrl } from "@/lib/open-url";
import { formatAbsolute, labelForSource, severityLabel, severityToneClass } from "./alert-display";
import { parseAlertDetailRecord, parseDeployRegressionDetail } from "./alert-detail-model";
import { AlertRegressionBlame } from "./AlertRegressionBlame";
import { toNavPage, type NavTarget } from "@/components/layout/nav-page";

interface Props {
  alert: AlertRow;
  onMarkViewed: () => void;
  onMarkUnread: () => void;
  onDismiss: () => void;
  onNavigate?: (page: NavTarget) => void;
  onClose?: () => void;
}

export function AlertDossier({
  alert,
  onMarkViewed,
  onMarkUnread,
  onDismiss,
  onNavigate,
  onClose,
}: Props) {
  const detailRecord = parseAlertDetailRecord(alert.detailJson);
  const regressionDetail = parseDeployRegressionDetail(detailRecord);
  const primaryAction = getPrimaryAction(detailRecord);
  const externalAction = getExternalAction(detailRecord);
  const recommendedAction = getRecommendedAction(alert, detailRecord);
  const severityText = severityLabel(alert.severity);
  const actions: AlertAction[] = [
    ...(primaryAction && onNavigate
      ? [
          {
            key: "primary",
            label: primaryAction.label,
            icon: <ChevronRight className="icon-md" />,
            onClick: () => onNavigate(toNavPage(primaryAction.page)),
            variant: "default" as const,
          },
        ]
      : []),
    ...(externalAction
      ? [
          {
            key: "external",
            label: externalAction.label,
            icon: <ExternalLink className="icon-md" />,
            onClick: () => void openUrl(externalAction.url),
            variant: "outline" as const,
          },
        ]
      : []),
    alert.viewedAt === null
      ? {
          key: "mark-read",
          label: "Mark read",
          icon: <CheckCheck className="icon-md" />,
          onClick: onMarkViewed,
          variant: "outline" as const,
        }
      : {
          key: "mark-unread",
          label: "Mark unread",
          icon: <RotateCcw className="icon-md" />,
          onClick: onMarkUnread,
          variant: "outline" as const,
        },
    alert.dismissedAt === null
      ? {
          key: "dismiss",
          label: "Dismiss",
          icon: <XCircle className="icon-md" />,
          onClick: onDismiss,
          variant: "outline" as const,
        }
      : {
          key: "dismissed",
          label: "Dismissed",
          icon: <XCircle className="icon-md" />,
          disabled: true,
          variant: "outline" as const,
        },
  ];
  const closeDossier = onClose ?? (() => {});

  const leftRail = (
    <DossierRail className="dossier-rail-section-plain">
      <div className="dossier-rail-list">
        <div className="dossier-rail-row">
          <span className="dossier-rail-row-key">Occurred</span>
          <span className="dossier-rail-row-value">{formatAbsolute(alert.occurredAt)}</span>
        </div>
        <div className="dossier-rail-row">
          <span className="dossier-rail-row-key">Last seen</span>
          <span className="dossier-rail-row-value">{formatAbsolute(alert.lastSeenAt)}</span>
        </div>
      </div>
    </DossierRail>
  );

  const rightRail = (
    <DossierRail>
      <div className="dossier-rail-button-stack">
        {actions.map((action) => (
          <Button
            key={action.key}
            variant={action.variant}
            onClick={action.onClick}
            disabled={action.disabled}
            aria-label={action.label}>
            {action.icon}
            <span>{action.label}</span>
          </Button>
        ))}
      </div>
    </DossierRail>
  );

  return (
    <IssueDossierPanel
      title={alert.title}
      subtitle={alert.description}
      eyebrow={
        <>
          <span className={severityToneClass(alert.severity)}>{severityText}</span>
          {` - ${labelForSource(alert.source)}`}
        </>
      }
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={closeDossier}>
      {regressionDetail ? (
        <DossierNumberedSection label="What Your Deploy Changed" tone="attention">
          <AlertRegressionBlame
            detail={regressionDetail}
            onOpenIssues={onNavigate ? () => onNavigate("issues") : undefined}
          />
        </DossierNumberedSection>
      ) : null}
      <DossierNumberedSection label="Recommended Action" tone="action">
        <p className="body-text">{recommendedAction}</p>
      </DossierNumberedSection>
    </IssueDossierPanel>
  );
}

interface AlertAction {
  key: string;
  label: string;
  icon: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant: "default" | "outline";
}

function getPrimaryAction(
  details: Record<string, unknown>,
): { page: string; label: string } | null {
  const destination = stringDetail(details.destination);
  if (!destination) return null;
  const page = DESTINATION_TO_PAGE[destination];
  if (!page) return null;
  return { page, label: PAGE_ACTION_LABEL[page] ?? "Open Related Page" };
}

function getExternalAction(
  details: Record<string, unknown>,
): { url: string; label: string } | null {
  const advisoryUrl = stringDetail(details.advisory_url);
  if (advisoryUrl) return { url: advisoryUrl, label: "View Advisory" };
  const htmlUrl = stringDetail(details.html_url);
  if (htmlUrl) return { url: htmlUrl, label: "View Source Event" };
  const externalUrl = stringDetail(details.external_url);
  if (externalUrl) return { url: externalUrl, label: "Open External Link" };
  return null;
}

function getRecommendedAction(alert: AlertRow, details: Record<string, unknown>): string {
  const alertType = stringDetail(details.alert_type);
  switch (alertType) {
    case "web_score_drop":
    case "web_critical_increase":
    case "web_first_critical":
      return "Open Issues and compare the newest Web Scan findings against the previous scan. Fix new critical or high-confidence items first, and dismiss only findings that are genuinely not applicable.";
    case "code_score_drop":
    case "code_critical_increase":
    case "code_first_critical":
      return "Open Issues and review the linked-project findings that changed in the latest Code Scan. Prioritize critical security, data-loss, auth, and production-readiness findings before shipping more code.";
    case "scan_failed":
      return "Check the target URL or linked project path, read the source error, then rerun the scan. Do not treat missing findings as healthy until SiteCMD completes a fresh scan.";
    case "uptime_monitor_down":
      return "Verify the monitored URL from outside your local machine, then check hosting, DNS, deploy status, and the monitor error details. Keep the alert open until the service is reachable again.";
    case "cloudflare_threats_blocked":
      return "Open Cloudflare Security Events for the same period and look for concentrated IPs, rules, paths, or countries. Treat it as review context before changing firewall or rate-limit rules.";
    case "plausible_traffic_drop":
    case "ga4_traffic_drop":
      return "Compare the drop with recent deploys, outages, campaigns, tracking changes, and Search Console movement. Verify the analytics tag still fires before assuming demand actually fell.";
    case "plausible_traffic_spike":
    case "ga4_traffic_spike":
      return "Check campaigns, referral sources, bot traffic, and deploy history. If the spike is legitimate, make sure the site and conversion flow can handle the added traffic.";
    case "gsc_query_impression_drop":
      return "Open Search Console for the query and affected pages. Check indexing, canonical selection, recent content changes, position movement, and seasonality before rewriting content.";
    case "security_update":
      return "Update the affected package, review the advisory scope, and run the relevant test/build path before deploying. If the package is runtime-facing, treat the fix as higher priority.";
    case "ssl_expiring":
      return "Confirm whether renewal is automatic. If it is not, renew and install the certificate now, then re-check the HTTPS chain after issuance.";
    case "ci_failure":
      return "Open the failed GitHub Actions run, fix the earliest failing job, and rerun CI before deploying or tagging a release.";
    case "deploy_regression":
      return "Open Issues and fix the introduced findings - each dossier has a fix guide and the agent handoff. The commit range above is the blame window to hand your AI tool if you want it to investigate the change itself.";
    default:
      return getSourceRecommendedAction(alert.source);
  }
}

function getSourceRecommendedAction(source: string): string {
  if (source === "uptimerobot") {
    return "Check the monitored URL, then use the connected service details to confirm whether the outage is still active before dismissing the alert.";
  }
  if (source === "cloudflare") {
    return "Review the Cloudflare event details and compare them with deploy, traffic, and security activity before changing protection rules.";
  }
  if (source === "plausible" || source === "ga4") {
    return "Compare the analytics change with deploys, campaigns, tracking changes, outages, and search movement before deciding whether action is needed.";
  }
  if (source === "gsc") {
    return "Open Search Console and inspect the affected query or page trend before making SEO changes.";
  }
  if (source === "github") {
    return "Open the source event, fix the failing workflow or deployment signal, and verify it is green before release work continues.";
  }
  if (source === "updates") {
    return "Open Updates or Security, apply the recommended change, and verify with a fresh check before dismissing the alert.";
  }
  return "Use the available context to confirm the event is still relevant, then open the related SiteCMD surface and verify the change with fresh data before dismissing it.";
}

function stringDetail(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

const DESTINATION_TO_PAGE: Record<string, string> = {
  activity: "events",
  analytics: "analytics",
  deploys: "deploys",
  events: "events",
  integrations: "integrations",
  issues: "issues",
  security: "security",
  search: "search-console",
  search_console: "search-console",
  "search-console": "search-console",
  settings: "settings",
  traffic: "analytics",
  updates: "updates",
};

const PAGE_ACTION_LABEL: Record<string, string> = {
  analytics: "Open Traffic",
  deploys: "Open Deploys",
  events: "Open Activity",
  integrations: "Open Integrations",
  issues: "Open Issues",
  security: "Open Security",
  "search-console": "Open Search & SEO",
  settings: "Open Settings",
  updates: "Open Updates",
};
