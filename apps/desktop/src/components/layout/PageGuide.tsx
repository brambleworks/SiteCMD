import { useCallback, useEffect, useId, useRef, useState, type RefObject } from "react";
import { CircleHelp, X } from "lucide-react";
import type { NavPage } from "@/components/layout/nav-page";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

interface PageGuideSection {
  title: string;
  items: readonly string[];
}

interface PageGuideContent {
  title: string;
  summary: string;
  sections: readonly PageGuideSection[];
}

export type PageGuideKey = NavPage;

const PAGE_GUIDES: Record<PageGuideKey, PageGuideContent> = {
  dashboard: {
    title: "Site Dashboard",
    summary:
      "See the selected site's current health, highest-priority work, key signals, and recent activity.",
    sections: [
      {
        title: "Start here",
        items: [
          "Confirm the site and environment in the identity strip, then check the Issues and Updates cards for work that needs attention.",
          "Use At a Glance for the SiteCMD Score, uptime, visitors, and search clicks. A missing metric means its source is not available.",
          "Read Recent Activity when a score or connected signal changes unexpectedly.",
        ],
      },
      {
        title: "Go deeper",
        items: [
          "Open Issues, Updates, or Alerts when an item needs investigation or action.",
          "Run a new scan after changing the site or linked code so the Dashboard reflects the current state.",
          "Connect services in Integrations or link a project folder in Site Setup when the Dashboard lacks context.",
        ],
      },
    ],
  },
  analytics: {
    title: "Traffic & Uptime",
    summary:
      "Review visitor, uptime, CDN, and delivery data available from the services connected to this project.",
    sections: [
      {
        title: "Read the data",
        items: [
          "Choose the period first, then use Traffic Summary and Recent Traffic to compare demand over the same window.",
          "Traffic Sources and Top Pages show where visits came from and which paths received them.",
          "Use the uptime, response-time, and Cloudflare sections to separate availability or delivery problems from traffic changes.",
        ],
      },
      {
        title: "Check coverage",
        items: [
          "Open Sources to see which providers contribute data. A blank section means no available provider returned that kind of data for the selected period.",
          "Connect Plausible, Google Analytics, Cloudflare, or UptimeRobot in Integrations when a signal is missing.",
        ],
      },
    ],
  },
  issues: {
    title: "Issues",
    summary:
      "Review Web Scan and Code Scan findings, decide what to fix, and verify completed work.",
    sections: [
      {
        title: "Work the list",
        items: [
          "Use the status, source, severity, and category filters to narrow the active findings.",
          "Open a finding for evidence, impact, fix guidance, and relevant files when they are available.",
          "Pages groups findings by URL. History shows past scan runs and how their totals changed.",
        ],
      },
      {
        title: "Take action",
        items: [
          "Use Fix with Agent or Batch prompt to hand selected work to your coding tool, then Verify fix after the change is made.",
          "Ignore a finding only when you accept it for this project. Block it when work cannot continue yet.",
          "Reopen an ignored or blocked finding when it needs attention again.",
        ],
      },
    ],
  },
  alerts: {
    title: "Alerts",
    summary:
      "Review outages, regressions, threats, service failures, and unusual changes that need timely attention.",
    sections: [
      {
        title: "Review alerts",
        items: [
          "Use All, Unread, Viewed, and Dismissed to separate new alerts from ones already reviewed.",
          "Open an alert to see its source and available next action. Mark all read clears the unread state in one step.",
          "Dismiss removes an alert from active attention; it does not resolve the issue or service condition that created it.",
        ],
      },
      {
        title: "Check coverage",
        items: [
          "Connected Alerts shows alerts delivered by the connected service for this project.",
          "Sources separates built-in checks from connected providers and shows which services are enabled.",
          "Use Check sources when coverage looks stale, or Open Integrations to repair a provider.",
        ],
      },
    ],
  },
  deploys: {
    title: "Deployments",
    summary:
      "Compare local commits, GitHub CI and pull requests, and the latest Web Scan to find unverified changes or likely regressions.",
    sections: [
      {
        title: "Review recent changes",
        items: [
          "Total Commits, Success Rate, and Last Web Scan summarize the current change and verification state.",
          "Pending commits since the last scan means the linked repository has moved ahead of the last verified site state.",
          "Latest Commits shows the local history. Connect GitHub to add workflow runs and open pull requests.",
        ],
      },
      {
        title: "Connect changes to outcomes",
        items: [
          "A regression callout appears when SiteCMD can correlate a deploy event with a later score drop or finding change. Open the affected Issues for evidence.",
          "Use Scan after deploy on a relevant commit to verify the live site after a production change.",
        ],
      },
    ],
  },
  updates: {
    title: "Updates",
    summary:
      "Find outdated or vulnerable packages, prepare update commands, and verify changes made in the linked repository.",
    sections: [
      {
        title: "Choose the work",
        items: [
          "Review security updates first, then use All, Major, Minor, and Patch to narrow the remaining packages.",
          "Open an update to see the installed and available versions, security details, and the package-manager command.",
          "Handle major framework, build, database, authentication, and payment updates as separate compatibility checks.",
        ],
      },
      {
        title: "Make and verify changes",
        items: [
          "Copy a package command, use Copy All Commands, or choose Fix with Agent when that action is available.",
          "SiteCMD prepares or hands off the work; it does not install packages directly in the Updates page.",
          "Recent Dependency Changes lists package follow-ups SiteCMD can re-check. Use Verify after the repository has changed, or Ignore or Block work that should not proceed.",
        ],
      },
    ],
  },
  events: {
    title: "Activity",
    summary:
      "Review recorded scan, verification, change, and monitoring events in chronological or calendar views.",
    sections: [
      {
        title: "Choose a view",
        items: [
          "Feed shows recent events in order. Day, Week, and Month place the same history on a calendar.",
          "Use Scans & verification, Changes, and Monitoring to focus the timeline on the event types you need.",
          "Calendar views include date navigation and a Today shortcut. Feed covers the recent 30-day window.",
        ],
      },
      {
        title: "Use the record",
        items: [
          "Open linked events to move from the timeline to the related page or finding.",
          "Export JSON or CSV when you need the visible range outside SiteCMD.",
          "Activity shows up to 500 events for a selected range and tells you when more history exists.",
        ],
      },
    ],
  },
  "search-console": {
    title: "Search & SEO",
    summary:
      "Review Google and Bing search visibility data for the selected site and verify recent search-related changes.",
    sections: [
      {
        title: "Read search visibility",
        items: [
          "Google Search Visibility shows clicks, impressions, click-through rate, average position, queries, and pages for the selected period.",
          "Bing Search Visibility shows clicks, impressions, average position, crawl errors, queries, and pages.",
          "Use Refresh search data after changing a provider or when the displayed results are stale.",
        ],
      },
      {
        title: "Resolve missing or pending data",
        items: [
          "Connect or repair Google Search Console and Bing Webmaster Tools from the setup cards when a provider is missing.",
          "Recent SEO Changes lists search-related follow-ups that still need a fresh check.",
          "Web Scan findings for metadata, robots, sitemaps, redirects, and canonicals remain in Issues.",
        ],
      },
    ],
  },
  integrations: {
    title: "Integrations",
    summary:
      "Connect AI agents for fixes and services that add traffic, uptime, search, deployment, and issue-tracker context.",
    sections: [
      {
        title: "Connect an agent",
        items: [
          "Connect Claude Code, Codex, Cursor, or Windsurf so the agent can read SiteCMD issues, make changes, and request verification.",
          "SiteCMD shows the planned MCP configuration change before writing it and can repair an outdated registration.",
          "Use Manual setup when automatic detection or configuration is not available for your tool.",
        ],
      },
      {
        title: "Connect services",
        items: [
          "Add analytics, monitoring, search, GitHub, or Jira connections that match the signals this project uses.",
          "Open Active connections to manage configured providers. Service credentials stay on this machine and are used only to communicate directly with that provider.",
        ],
      },
    ],
  },
  sites: {
    title: "Overview",
    summary: "Compare tracked projects, choose the site that needs attention, and switch projects.",
    sections: [
      {
        title: "Compare sites",
        items: [
          "Total Sites, Active Issues, Avg. SiteCMD Score, and Scanned Sites summarize the portfolio.",
          "The average uses only sites with a current score; unscanned sites are not counted as zero.",
          "Each row shows the site's current SiteCMD Score and active issue count. Critical findings change the issue count's emphasis.",
        ],
      },
      {
        title: "Open or add a site",
        items: [
          "Select a row to make that project active before running scans, changing settings, or building reports. The current-site treatment shows which project is already active.",
          "Use Add Site when another property needs its own URL, history, linked folder, and integrations.",
        ],
      },
    ],
  },
  settings: {
    title: "Project Settings",
    summary: "Manage the selected project and workspace-wide SiteCMD behavior.",
    sections: [
      {
        title: "This Project",
        items: [
          "Site Setup manages the project name, linked folder, environment URLs, sitemap pages, and project removal.",
          "Scanning controls scan preferences and schedules. Automation contains CI/CD and webhook delivery settings.",
          "Connected shows the selected project's connected-service state and related controls.",
        ],
      },
      {
        title: "Workspace",
        items: [
          "Account & Billing manages the account and license. App Preferences controls theme, desktop behavior, notifications, and updates.",
          "Privacy & Diagnostics controls telemetry, crash reporting, and diagnostic data. Data manages the local database, backups, and cleanup.",
          "Use Integrations, not Settings, to connect agent tools and external data providers.",
        ],
      },
    ],
  },
  reports: {
    title: "Reports",
    summary:
      "Build a current report from scan results and available connected data, then preview or export it.",
    sections: [
      {
        title: "Configure the report",
        items: [
          "Set the report title and choose the 7-day, 30-day, or 90-day reporting period.",
          "Report Coverage and Latest Included Snapshot show which Web Scan, Code Scan, and connected signals are available.",
          "Choose the included sections and expand Branding when the report needs a company name, logo, client name, colors, or footer text.",
        ],
      },
      {
        title: "Preview and export",
        items: [
          "Refresh stale scan or connected data first. Generate Report opens a preview built from the current configuration and latest included snapshot.",
          "Use Export PDF or Save HTML from the preview. Report History keeps earlier generated reports available for review or regeneration.",
        ],
      },
    ],
  },
};

export function PageGuideButton({ page, className }: { page: PageGuideKey; className?: string }) {
  const guide = PAGE_GUIDES[page];
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const guideLabel = `${guide.title} guide`;

  const handleClose = useCallback(() => {
    setOpen(false);
  }, []);

  return (
    <>
      <Button
        unstyled
        ref={triggerRef}
        type="button"
        onClick={() => setOpen(true)}
        className={cn("page-guide-trigger", className)}
        aria-label={`Open ${guideLabel}`}
        title={`Open ${guideLabel}`}>
        <CircleHelp className="icon-md" aria-hidden="true" />
        <span className="page-guide-trigger__label">Guide</span>
      </Button>
      {open ? <PageGuidePanel guide={guide} onClose={handleClose} triggerRef={triggerRef} /> : null}
    </>
  );
}

function PageGuidePanel({
  guide,
  onClose,
  triggerRef,
}: {
  guide: PageGuideContent;
  onClose: () => void;
  triggerRef: RefObject<HTMLButtonElement | null>;
}) {
  const [visible, setVisible] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const closeTimerRef = useRef<number | null>(null);
  const onCloseRef = useRef(onClose);
  const titleId = useId();
  const summaryId = useId();

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  const requestClose = useCallback(() => {
    setVisible(false);
    if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      onCloseRef.current();
    }, 180);
  }, []);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      setVisible(true);
      closeButtonRef.current?.focus();
    });
    return () => {
      window.cancelAnimationFrame(frame);
      if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    };
  }, []);

  return (
    <Dialog
      labelledBy={titleId}
      describedBy={summaryId}
      onClose={requestClose}
      restoreFocusTo={triggerRef}
      className={cn(
        "page-guide-panel",
        visible ? "page-guide-panel-visible" : "page-guide-panel-hidden",
      )}>
      <div className="page-guide-header">
        <div className="flex-fill">
          <h2 id={titleId} className="details-title page-guide-title">
            {guide.title}
          </h2>
          <p id={summaryId} className="details-subtitle">
            {guide.summary}
          </p>
        </div>
        <Button
          unstyled
          ref={closeButtonRef}
          type="button"
          onClick={requestClose}
          aria-label="Close page guide"
          className="details-close">
          <X aria-hidden="true" />
        </Button>
      </div>

      <div className="page-guide-body">
        {guide.sections.map((section) => (
          <GuideSection key={section.title} section={section} />
        ))}
      </div>
    </Dialog>
  );
}

function GuideSection({ section }: { section: PageGuideSection }) {
  return (
    <section className="page-guide-section stack-base">
      <h3 className="text-body text-strong">{section.title}</h3>
      <GuideList items={section.items} />
    </section>
  );
}

function GuideList({ items }: { items: readonly string[] }) {
  return (
    <ul className="page-guide-list">
      {items.map((item) => (
        <li key={item}>{item}</li>
      ))}
    </ul>
  );
}
