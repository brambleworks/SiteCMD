import { useState, useMemo } from "react";
import type { AppTarget } from "@/lib/app-targets";
import { parseJsonRecord } from "@/lib/json-record";
import type { ProjectWorkSummary } from "@/lib/project-summary-signals";
import type { SiteEvent } from "@/lib/types";
import { getScoreClass } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  getPrimaryWorkSummaryCue,
  getWorkSummaryBadgeClassName,
  getWorkSummaryBadgeTarget,
  getWorkSummaryBadges,
  readPersistedWorkSummaryCue,
  type WorkSummaryBadgeKey,
} from "@/lib/work-item-presentation";
import {
  buildEventScanTarget,
  getEventOpenLabel,
  getUnhandledEventDetailEntries,
  humanizeEventDetail,
} from "./event-presentation";
import {
  Shield,
  Gauge,
  Search,
  Eye,
  GitCommit,
  Wifi,
  BarChart3,
  Settings,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  Clock,
  Users,
  Activity,
} from "lucide-react";

interface DayViewProps {
  date: Date;
  events: SiteEvent[];
  workSummary: ProjectWorkSummary | null;
  onOpenTarget?: (target: AppTarget) => void;
}

const TYPE_ICON: Record<string, typeof Shield> = {
  scan: Search,
  verification: RefreshCw,
  update: RefreshCw,
  launch: Shield,
  deploy: GitCommit,
  uptime: Wifi,
  analytics: BarChart3,
  security: Shield,
  performance: Gauge,
  accessibility: Eye,
  compliance: Settings,
};

// Shared event variables keep the day, week, month, and activity colors aligned.
const TYPE_DOT: Record<string, string> = {
  scan: "event-bar--scan",
  verification: "event-bar--verification",
  update: "event-bar--update",
  launch: "event-bar--launch",
  deploy: "event-bar--deploy",
  uptime: "event-bar--uptime",
  analytics: "event-bar--analytics",
  security: "event-bar--security",
  performance: "event-bar--performance",
  accessibility: "event-bar--accessibility",
  compliance: "event-bar--compliance",
};

const TYPE_BORDER: Record<string, string> = {
  scan: "event-border--scan",
  verification: "event-border--verification",
  update: "event-border--update",
  launch: "event-border--launch",
  deploy: "event-border--deploy",
  uptime: "event-border--uptime",
  analytics: "event-border--analytics",
  security: "event-border--security",
  performance: "event-border--performance",
  accessibility: "event-border--accessibility",
  compliance: "event-border--compliance",
};

const SEV_LABEL: Record<string, { text: string; cls: string }> = {
  critical: { text: "Critical", cls: "text-severity-critical" },
  warning: { text: "Warning", cls: "text-severity-medium" },
  info: { text: "Info", cls: "text-brand" },
};

function fmtTime(ms: number): string {
  try {
    return new Date(ms).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  } catch {
    return "";
  }
}

function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function parseDetail(evt: SiteEvent): Record<string, unknown> | null {
  if (evt.parsedDetail !== undefined) return evt.parsedDetail;
  if (!evt.detail) return null;
  return parseJsonRecord(evt.detail);
}

export function DayView({ date, events, workSummary, onOpenTarget }: DayViewProps) {
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const key = dateKey(date);

  const dayEvents = useMemo(
    () =>
      events
        .filter((e) => dateKey(new Date(e.occurredAtMs)) === key)
        .sort((a, b) => a.occurredAtMs - b.occurredAtMs),
    [events, key],
  );

  const summary = useMemo(() => {
    const stats: {
      webScanScore: number | null;
      visitors: number | null;
      pageviews: number | null;
      uptimeStatus: string | null;
      deploys: number;
      threats: number | null;
    } = {
      webScanScore: null,
      visitors: null,
      pageviews: null,
      uptimeStatus: null,
      deploys: 0,
      threats: null,
    };

    for (const evt of dayEvents) {
      const detail = parseDetail(evt);
      if (!detail) continue;

      if (evt.eventType === "scan" && typeof detail.score === "number") {
        // Use the latest scan artifact score.
        stats.webScanScore = detail.score as number;
      }

      if (evt.eventType === "analytics") {
        if (typeof detail.visitors === "number") stats.visitors = detail.visitors as number;
        if (typeof detail.pageviews === "number") stats.pageviews = detail.pageviews as number;
      }

      if (evt.eventType === "uptime") {
        const logType = detail.log_type as number;
        if (logType === 1) stats.uptimeStatus = "down";
        else if (logType === 2 && stats.uptimeStatus !== "down") stats.uptimeStatus = "recovered";
      }

      if (evt.eventType === "deploy") stats.deploys++;

      if (evt.eventType === "security" && typeof detail.threats === "number") {
        stats.threats = (stats.threats || 0) + (detail.threats as number);
      }
    }

    return stats;
  }, [dayEvents]);

  const hasStats =
    summary.webScanScore !== null ||
    summary.visitors !== null ||
    summary.uptimeStatus !== null ||
    summary.deploys > 0 ||
    summary.threats !== null;

  if (dayEvents.length === 0) {
    return (
      <div className="empty-state day-empty">
        <Clock className="day-empty-icon" />
        <p className="empty-state-title text-muted-foreground">No events</p>
        <p className="muted-text day-empty-sub">Nothing happened on this day - yet.</p>
      </div>
    );
  }

  return (
    <div className="page-content">
      {hasStats && (
        <div className="day-stats-grid">
          {summary.webScanScore !== null && (
            <div className="card-section-sm">
              <div className="row-tight section-label day-stat-head">
                <Activity className="icon-xs" />
                Web Scan
              </div>
              <span className={`stat-value ${getScoreClass(summary.webScanScore)}`}>
                {summary.webScanScore}
              </span>
              <span className="muted-text">/100</span>
            </div>
          )}

          {summary.visitors !== null && (
            <div className="card-section-sm">
              <div className="row-tight section-label day-stat-head">
                <Users className="icon-xs" />
                Visitors
              </div>
              <span className="stat-value">{summary.visitors.toLocaleString()}</span>
              {summary.pageviews !== null && (
                <span className="muted-text day-stat-unit">
                  {summary.pageviews.toLocaleString()} views
                </span>
              )}
            </div>
          )}

          {summary.deploys > 0 && (
            <div className="card-section-sm">
              <div className="row-tight section-label day-stat-head">
                <GitCommit className="icon-xs" />
                Deploys
              </div>
              <span className="stat-value">{summary.deploys}</span>
              <span className="muted-text day-stat-unit">
                commit{summary.deploys !== 1 ? "s" : ""}
              </span>
            </div>
          )}

          {summary.uptimeStatus !== null && (
            <div className="card-section-sm">
              <div className="row-tight section-label day-stat-head">
                <Wifi className="icon-xs" />
                Uptime
              </div>
              <span
                className={`day-uptime-status ${
                  summary.uptimeStatus === "down"
                    ? "text-severity-critical"
                    : "text-score-excellent"
                }`}>
                {summary.uptimeStatus === "down" ? "Downtime detected" : "Recovered"}
              </span>
            </div>
          )}

          {summary.threats !== null && summary.threats > 0 && (
            <div className="card-section-sm">
              <div className="row-tight section-label day-stat-head">
                <Shield className="icon-xs" />
                Threats
              </div>
              <span className="day-threats-value text-brand-accent">{summary.threats}</span>
              <span className="muted-text day-stat-unit">blocked</span>
            </div>
          )}
        </div>
      )}

      <div className="day-timeline">
        <div className="timeline-rail" />

        <div className="stack-base">
          {dayEvents.map((evt) => {
            const Icon = TYPE_ICON[evt.eventType] || Settings;
            const dot = TYPE_DOT[evt.eventType] || "event-bar--muted";
            const border = TYPE_BORDER[evt.eventType] || "event-border--muted";
            const sev = SEV_LABEL[evt.severity] || SEV_LABEL.info;
            const open = expandedId === evt.id;
            const time = fmtTime(evt.occurredAtMs);

            const detail = parseDetail(evt);
            const detailPills = detail ? humanizeEventDetail(detail, evt.eventType) : [];
            const eventTarget = buildEventScanTarget(evt.projectId, detail);
            const rawDetailEntries = detail ? getUnhandledEventDetailEntries(detail) : [];
            const showWorkflowNow = Boolean(
              workSummary && ["scan", "security", "accessibility"].includes(evt.eventType),
            );
            const activeWorkSummary = showWorkflowNow ? workSummary : null;
            const persistedWorkflowCue = readPersistedWorkSummaryCue(detail);
            const workflowBadges = activeWorkSummary
              ? getWorkSummaryBadges(activeWorkSummary).slice(0, 3)
              : [];
            const primaryWorkflowCue = activeWorkSummary
              ? getPrimaryWorkSummaryCue(activeWorkSummary)
              : null;

            return (
              <div key={evt.id} className="day-timeline-item">
                <div className="day-event-time">
                  <span className="meta-num">{time}</span>
                </div>

                <div className={`day-timeline-dot ${dot}`} />

                <div className={`day-event-card ${border}`}>
                  <Button
                    unstyled
                    onClick={() => setExpandedId(open ? null : evt.id)}
                    className="timeline-event-trigger">
                    <div className="day-event-icon">
                      <Icon className="icon-sm text-muted-foreground" />
                    </div>
                    <div className="flex-fill">
                      <div className="row">
                        <span className="text-body day-event-title">{evt.title}</span>
                        <span className={`day-event-sev ${sev.cls}`}>{sev.text}</span>
                      </div>
                      <p className="muted-text day-event-summary">{evt.summary}</p>
                    </div>
                    <div className="day-event-chevron">
                      {open ? (
                        <ChevronDown className="icon-md" />
                      ) : (
                        <ChevronRight className="icon-md" />
                      )}
                    </div>
                  </Button>

                  {open && detail && (
                    <div className="day-event-detail">
                      {(detailPills.length > 0 || (eventTarget && onOpenTarget)) && (
                        <div className="day-detail-pills">
                          {detailPills.map((pill, index) => (
                            <span key={`${evt.id}-pill-${index}`} className="event-chip">
                              {pill}
                            </span>
                          ))}
                          {eventTarget && onOpenTarget ? (
                            <Button
                              unstyled
                              onClick={() => onOpenTarget(eventTarget)}
                              className="day-detail-link">
                              {getEventOpenLabel(eventTarget)} →
                            </Button>
                          ) : null}
                        </div>
                      )}
                      {rawDetailEntries.length > 0 && (
                        <div className="day-detail-grid">
                          {rawDetailEntries.map(([k, v]) => (
                            <div key={k} className="day-detail-entry text-meta">
                              <span className="text-muted-foreground day-detail-key">{k}</span>
                              <span className="day-detail-val text-truncate">{String(v)}</span>
                            </div>
                          ))}
                        </div>
                      )}
                      {(persistedWorkflowCue || (showWorkflowNow && primaryWorkflowCue)) && (
                        <div className="day-workflow">
                          <p className="text-micro day-workflow-label">
                            {persistedWorkflowCue ? "Workflow Then" : "Workflow Now"}
                          </p>
                          <p className="text-meta day-workflow-text">
                            {persistedWorkflowCue?.sentence ?? primaryWorkflowCue?.sentence}
                          </p>
                          {persistedWorkflowCue && (
                            <div className="day-workflow-badges">
                              <span
                                className={`day-workflow-badge ${getWorkSummaryBadgeClassName(persistedWorkflowCue.key)}`}>
                                {persistedWorkflowCue.label}
                              </span>
                            </div>
                          )}
                          {workflowBadges.length > 0 && (
                            <div className="day-workflow-badges">
                              {workflowBadges.map((badge) => {
                                const target = getWorkSummaryBadgeTarget(
                                  activeWorkSummary!,
                                  badge.key as WorkSummaryBadgeKey,
                                );
                                if (target && onOpenTarget) {
                                  return (
                                    <Button
                                      unstyled
                                      key={badge.key}
                                      onClick={() => onOpenTarget(target)}
                                      className={`day-workflow-badge day-workflow-badge--button ${badge.className}`}>
                                      {badge.label}
                                    </Button>
                                  );
                                }
                                return (
                                  <span
                                    key={badge.key}
                                    className={`day-workflow-badge ${badge.className}`}>
                                    {badge.label}
                                  </span>
                                );
                              })}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
