import {
  ChevronRight,
  Eye,
  FileCode,
  RefreshCw,
  Search,
  Shield,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { getProjectCapabilities } from "@/lib/project-capabilities";
import { getHostname } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import type { NavTarget } from "@/components/layout/nav-page";

type SignalRow = {
  icon: LucideIcon;
  label: string;
  desc: string;
};

interface DashboardEmptyStateProps {
  url: string;
  projectName: string;
  framework: string | null;
  projectPath: string | null;
  onOpenScanConfig: () => void;
  onAddFolder: () => void;
  onNavigate: (page: NavTarget) => void;
}

export function DashboardLoadingState() {
  const glanceTiles = ["Web Scan", "Code Scan", "Critical Risk", "Uptime", "Visitors", "Search"];
  const referenceTiles = ["Web Vitals", "Search Indexing", "CDN & Caching", "Deploys"];

  return (
    <LoadingRegion label="Dashboard loading state" className="dash-loading">
      <section className="card card--spacious">
        <div className="row-between">
          <div className="dash-snap-copy">
            <p className="section-label-mid">Site Snapshot</p>
            <Skeleton variant="heading" width="md" />
            <p className="text-body-muted">
              Loading health, stack, scan freshness, and launch signals.
            </p>
          </div>
          <Skeleton variant="pill" width="sm" />
        </div>
        <div className="dash-snap-grid">
          {["Stack", "SSL", "Last scan"].map((label) => (
            <div key={label} className="dash-snap-cell bg-surface-low">
              <p className="section-label-mid">{label}</p>
              <Skeleton variant="line-lg" width="sm" />
            </div>
          ))}
        </div>
      </section>

      <section className="dash-glance-grid">
        {glanceTiles.map((label) => (
          <div key={label} className="card metric-card dash-metric-loading">
            <p className="section-label-mid">{label}</p>
            <Skeleton variant="stat" width="sm" />
            <Skeleton variant="line" width="sm" />
          </div>
        ))}
      </section>

      <section className="dash-attention-grid">
        {["What Needs Attention", "Recent Activity"].map((title) => (
          <div key={title} className="card">
            <div className="row-between">
              <p className="section-label-mid">{title}</p>
              <Skeleton variant="line" width="sm" />
            </div>
            <div className="dash-attention-list">
              {[0, 1, 2].map((row) => (
                <div key={row} className="dash-attention-row">
                  <Skeleton variant="dot" />
                  <div className="dash-attention-body">
                    <Skeleton variant="line-lg" width="lg" />
                    <Skeleton variant="line" width="md" />
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </section>

      <section className="dash-ref-grid">
        {referenceTiles.map((label) => (
          <div key={label} className="card metric-card dash-metric-loading">
            <p className="section-label-mid">{label}</p>
            <Skeleton variant="title" width="sm" />
            <Skeleton variant="line" width="md" />
          </div>
        ))}
      </section>

      <section className="card dash-whatsnext">
        <p className="section-label-mid">What's Next</p>
        <Skeleton variant="line-lg" width="lg" />
        <Skeleton variant="line" width="md" />
      </section>
    </LoadingRegion>
  );
}

export function DashboardEmptyState({
  url,
  projectName,
  framework,
  projectPath,
  onOpenScanConfig,
  onAddFolder,
  onNavigate,
}: DashboardEmptyStateProps) {
  // One scan covers whichever halves the project has, so the prompt describes
  // what this project's scan will actually do rather than always saying "web".
  const { hasSite, hasCode } = getProjectCapabilities({
    environmentUrl: url,
    projectFolder: projectPath,
  });
  const scanBlurb = hasSite
    ? hasCode
      ? "Checks your live site and your linked code in one pass: security, performance, SEO, accessibility, and code risks."
      : "Checks your live site for security, performance, SEO, and accessibility problems."
    : "Checks your linked code for database, security, AI-safety, architecture, and deploy risks.";

  const signalRows: SignalRow[] = [
    { icon: Shield, label: "Security", desc: "Vulnerabilities, headers, SSL, exposed data" },
    { icon: Zap, label: "Performance", desc: "Speed, caching, resource optimization" },
    { icon: Search, label: "SEO", desc: "Meta tags, structure, indexability" },
    { icon: Eye, label: "Accessibility", desc: "WCAG compliance, screen reader support" },
    {
      icon: FileCode,
      label: "Code Scan",
      desc: "Database, AI-safety, auth, architecture, and deploy guardrails",
    },
    { icon: RefreshCw, label: "Dependencies", desc: "Outdated packages, security patches" },
  ];

  return (
    <div className="dash-empty">
      <div>
        <div className="dash-empty-head">
          <h1 className="dash-empty-title text-foreground">{projectName}</h1>
          {framework && <span className="new-badge">{framework}</span>}
        </div>
        {hasSite ? <p className="text-body">{getHostname(url)}</p> : null}
      </div>

      <Button
        unstyled
        type="button"
        onClick={onOpenScanConfig}
        className="dashboard-empty-action dashboard-empty-action--scan">
        <div className="dash-action-row">
          <div className="dashboard-empty-action-icon dashboard-empty-action-icon--scan">
            <Search className="icon-2xl text-primary" />
          </div>
          <div className="dash-action-copy">
            <p className="text-lg-bold">Run your first scan</p>
            <p className="text-13-muted dash-action-desc">
              {scanBlurb} Then your Issues list opens with everything to fix.
            </p>
          </div>
          <ChevronRight className="icon-lg dash-action-chevron text-primary" />
        </div>
      </Button>

      {hasCode ? null : (
        <Button
          unstyled
          type="button"
          onClick={onAddFolder}
          className="dashboard-empty-action dashboard-empty-action--code">
          <div className="dash-action-row">
            <div className="dashboard-empty-action-icon dashboard-empty-action-icon--code">
              <FileCode className="icon-xl text-emerald-300" />
            </div>
            <div className="dash-action-copy">
              <p className="text-lg-bold">Link your project folder</p>
              <p className="text-13-muted dash-action-desc">
                Connect your local repo and every scan will cover code risks alongside the live
                site, in one workspace.
              </p>
            </div>
            <ChevronRight className="icon-lg dash-action-chevron text-emerald-300" />
          </div>
        </Button>
      )}

      <div className="panel panel--muted">
        <h2 className="text-15-bold dash-checks-title">What each scan checks</h2>
        <p className="body-desc-xs dash-checks-desc">
          Every finding rolls into your Issues list, ranked by severity, so you always know what to
          fix next.
        </p>
        <div className="dash-checks-grid">
          {signalRows.map((m) => (
            <div key={m.label} className="dash-check-cell bg-card">
              <m.icon className="icon-md dash-check-icon text-foreground" />
              <p className="text-12-semibold">{m.label}</p>
              <p className="body-desc-xs">{m.desc}</p>
            </div>
          ))}
        </div>
      </div>

      <p className="text-13-muted">
        Analytics, uptime, search, and GitHub are optional. Connect them anytime from{" "}
        <Button
          unstyled
          type="button"
          onClick={() => onNavigate("integrations")}
          className="link-text">
          Integrations
        </Button>{" "}
        to layer live signals onto your dashboard.
      </p>
    </div>
  );
}
