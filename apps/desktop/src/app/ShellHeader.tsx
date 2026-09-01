import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { NavPage } from "@/components/layout/NavSidebar";
import { PageGuideButton } from "@/components/layout/PageGuide";
import { PageSkeleton, type PageSkeletonLayout } from "@/components/ui/page-skeleton";

const PAGE_HEADERS: Partial<Record<NavPage, { title: string; subtitle: string }>> = {
  dashboard: {
    title: "Site Dashboard",
    subtitle: "Start here for the site's current state, newest signals, and next best action.",
  },
  analytics: {
    title: "Traffic & Uptime",
    subtitle:
      "Visitor, reliability, and delivery signals from the services connected to this project.",
  },
  issues: {
    title: "Issues",
    subtitle:
      "Everything the scans found, ranked by what to fix first, plus what got worse, what is on hold, and past scans.",
  },
  deploys: {
    title: "Deployments",
    subtitle: "Recent commits, CI activity, and whether a deploy lines up with new site problems.",
  },
  updates: {
    title: "Updates",
    subtitle:
      "Outdated packages, vulnerable dependencies, and upgrade work that deserves a release check.",
  },
  events: {
    title: "Activity",
    subtitle: "A timeline of scans, deploys, monitoring signals, and verification work.",
  },
  "search-console": {
    title: "Search & SEO",
    subtitle: "Search visibility and crawl signals that affect whether people can find the site.",
  },
  integrations: {
    title: "Integrations",
    subtitle:
      "Connect AI agents for fixes and services for traffic, uptime, search, deploys, and issue tracking.",
  },
  settings: {
    title: "Project Settings",
    subtitle:
      "Manage this project's setup, scan behavior, schedule, delivery hooks, local history, and workspace preferences.",
  },
  reports: {
    title: "Reports",
    subtitle: "Build a shareable status report from current issues, proof, and connected signals.",
  },
  alerts: {
    title: "Alerts",
    subtitle:
      "Things that changed for the worse: outages, drops, new threats, and services that stopped working.",
  },
};

const HEADER_ACTIONS_ID = "shell-header-actions";

export function ShellPageHeader({
  page,
  showScanHeader,
}: {
  page: NavPage;
  showScanHeader: boolean;
}) {
  // Issues page header - Run Scan button is in the TopBar.
  if (showScanHeader) {
    return (
      <div className="shell-page-header">
        <div>
          <h1 className="page-title-lg">Issues</h1>
          <p className="text-13-muted shell-page-subtitle">
            Everything the scans found, ranked by what to fix first, plus what got worse, what is on
            hold, and past scans.
          </p>
        </div>
        <PageGuideButton page="issues" />
      </div>
    );
  }

  const header = PAGE_HEADERS[page];
  if (!header) return null;

  return (
    <div className="shell-page-header shell-page-header--end">
      <div>
        <h1 className="page-title-lg">{header.title}</h1>
        <p className="text-13-muted shell-page-subtitle">{header.subtitle}</p>
      </div>
      <div className="shell-header-actions">
        <div id={HEADER_ACTIONS_ID} className="shell-header-actions" />
        <PageGuideButton page={page} />
      </div>
    </div>
  );
}

export function ShellPageLoading({ page }: { page: NavPage }) {
  const layout: PageSkeletonLayout =
    page === "dashboard"
      ? "dashboard"
      : page === "issues" || page === "alerts"
        ? "split"
        : page === "events"
          ? "timeline"
          : page === "integrations" || page === "settings" || page === "sites"
            ? "cards"
            : "data";

  return <PageSkeleton label={`Loading ${PAGE_HEADERS[page]?.title ?? page}`} layout={layout} />;
}

export function HeaderActions({ children }: { children: ReactNode }) {
  const [target, setTarget] = useState<HTMLElement | null>(null);

  useEffect(() => {
    // Wait one frame for the portal target to mount.
    const raf = requestAnimationFrame(() => setTarget(document.getElementById(HEADER_ACTIONS_ID)));
    return () => cancelAnimationFrame(raf);
  }, []);

  if (!target) return null;
  return createPortal(children, target);
}
