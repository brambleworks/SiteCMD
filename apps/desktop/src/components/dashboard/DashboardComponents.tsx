/* eslint-disable react-refresh/only-export-components -- helpers are exported with components. */
import type { ReactNode } from "react";
import { AlertTriangle, ChevronRight, Plug, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ProgressBar } from "@/components/ui/progress-bar";
import { getScoreClass, getScoreLabel, type ScanCategory } from "@/lib/types";
import { CATEGORY_CSS_VAR, CATEGORY_LABELS } from "@/lib/tokens";
import type { NavTarget } from "@/components/layout/nav-page";

export {
  buildCategoryScores,
  CategoryTrendGrid,
  Sparkline,
} from "@/components/dashboard/DashboardTrendComponents";
export type { ScoreTrendPoint } from "@/components/dashboard/DashboardTrendComponents";
export { WebIssueDossier } from "@/components/issues/WebIssueDossier";
export { IssueListOverlay } from "@/components/dashboard/DashboardIssueListOverlay";
export { getSeverityConfig } from "@/components/dashboard/dashboard-issue-severity";

export function FirstScanBanner({
  score,
  issueCount,
  criticalCount,
  quickWinCount,
  onDismiss,
  onNavigate,
}: {
  score: number;
  issueCount: number;
  criticalCount: number;
  quickWinCount: number;
  onDismiss: () => void;
  onNavigate: (page: NavTarget) => void;
}) {
  const label = getScoreLabel(score);
  const nextSteps: { icon: ReactNode; title: string; desc: string; action: () => void }[] = [];

  if (criticalCount > 0) {
    nextSteps.push({
      icon: <AlertTriangle className="icon-md text-red-400" />,
      title: `Fix ${criticalCount} critical issue${criticalCount > 1 ? "s" : ""}`,
      desc: "These are the highest priority - security or performance problems that need immediate attention.",
      action: () => onNavigate("issues"),
    });
  }

  nextSteps.push({
    icon: <ChevronRight className="icon-md text-primary" />,
    title: "Open Issues",
    desc: "Go straight to the action center for the next fix, why it matters, and what to verify after you change it.",
    action: () => onNavigate("issues"),
  });

  nextSteps.push({
    icon: <Plug className="icon-md text-muted-foreground" />,
    title: "Connect integrations",
    desc: "Add analytics, uptime monitoring, and deploy tracking to your dashboard.",
    action: () => onNavigate("settings:integrations"),
  });

  const steps = nextSteps.slice(0, 3);

  return (
    <div className="card card--muted card--spacious">
      <div className="row-between-mb">
        <div className="stack-tight">
          <p className="text-15-bold">First scan complete</p>
          <p className="text-13-muted">
            Your SiteCMD Score is <span className="text-strong">{score}</span> - that's{" "}
            <span className={`first-scan-score-label ${getScoreClass(score)}`}>{label}</span>.
            {issueCount > 0 && (
              <>
                {" "}
                {issueCount} issue{issueCount !== 1 ? "s" : ""} found
                {quickWinCount > 0
                  ? ` - ${quickWinCount} are quick fixes you can knock out in minutes`
                  : ""}
                .
              </>
            )}
            {issueCount === 0 && <> No issues found - your site is in great shape.</>}
          </p>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={onDismiss}
          aria-label="Dismiss banner"
          className="first-scan-dismiss">
          <X className="icon-md" />
        </Button>
      </div>
      <p className="text-meta first-scan-steps-label">Recommended next steps</p>
      <div className="first-scan-steps-grid">
        {steps.map((step) => (
          <Button
            unstyled
            key={step.title}
            type="button"
            onClick={step.action}
            className="card card--compact card--interactive card--muted">
            <div className="first-scan-step-icon">{step.icon}</div>
            <p className="first-scan-step-title">{step.title}</p>
            <p className="body-desc-sm">{step.desc}</p>
          </Button>
        ))}
      </div>
    </div>
  );
}

export function VitalCard({
  icon,
  title,
  value,
  period,
  detail,
  valueColor,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  value: string;
  period: string;
  detail: string;
  valueColor?: string;
  onClick?: () => void;
}) {
  const Wrapper = onClick ? "button" : "div";
  return (
    <Wrapper
      className={`card card--compact card--muted metric-card vital-card ${onClick ? "card--interactive" : ""}`}
      onClick={onClick}>
      <div className="vital-card-head">
        {icon}
        <span className="text-meta vital-card-title">{title}</span>
        <span className="subtitle-xs vital-card-period">{period}</span>
      </div>
      <div className={`vital-card-value ${valueColor || ""}`}>{value}</div>
      <p className="subtitle-xs vital-card-detail">{detail}</p>
    </Wrapper>
  );
}

export function CategoryScoreCard({
  category,
  score,
  issues,
  onClick,
}: {
  category: ScanCategory;
  score: number;
  issues: number;
  onClick: () => void;
}) {
  const label = CATEGORY_LABELS[category] || category;
  const cssVar = CATEGORY_CSS_VAR[category] ?? "var(--brand)";

  return (
    <Button unstyled onClick={onClick} className="card card--compact card--interactive metric-card">
      <div className="category-score-head">
        <p className="text-meta category-score-label">
          {label}
          {issues > 0 ? (
            <span className="text-muted-foreground category-issue-count">· {issues}</span>
          ) : (
            ""
          )}
        </p>
        <span className="category-score-value">{score}%</span>
      </div>
      <ProgressBar percent={score} color={cssVar} trackClassName="progress-bar--thin" />
    </Button>
  );
}
