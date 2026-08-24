import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { CircleHelp, X } from "lucide-react";
import type { NavPage } from "@/components/layout/NavSidebar";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

interface PageGuideContent {
  title: string;
  subtitle: string;
  purpose: string;
  lookFirst: string[];
  useWell: string[];
  takeAction: string[];
  proTip: string;
}

/** Nav pages plus the score strip, which has a guide without being a page. */
export type PageGuideKey = NavPage | "score";

const PAGE_GUIDES: Record<PageGuideKey, PageGuideContent> = {
  dashboard: {
    title: "Site Dashboard Guide",
    subtitle: "A quick daily read on this site's condition and next move.",
    purpose:
      "The Dashboard is the fast answer to: is this site basically healthy, and what should I do next? It pulls the strongest signals from scans, connected services, and recent activity into one working view.",
    lookFirst: [
      "Start with the identity and health strip to see whether the selected site, environment, and latest scores look right.",
      "Check the action items before reading every metric. Those are the items most likely to change what you do today.",
      "Use recent activity to understand whether a scan, deploy, or integration event explains a sudden change.",
    ],
    useWell: [
      "Treat the Dashboard as triage, not the full investigation. Click into Issues, Updates, or Alerts when something needs work.",
      "After making changes, re-scan or refresh the connected signal so the Dashboard reflects the current site, not yesterday's state.",
      "If the page looks thin, connect more integrations or add the local project folder so SiteCMD has more context.",
    ],
    takeAction: [
      "A critical issue or security update appears in the action items.",
      "The SiteCMD Score drops after a deploy or content change.",
      "A connected service is stale and the Dashboard is missing a signal you rely on.",
    ],
    proTip:
      "For business owners, this is the morning check-in. For builders, this is where you decide which specialized page deserves attention.",
  },
  analytics: {
    title: "Traffic & Uptime Guide",
    subtitle: "Visitor demand, uptime, and delivery health in one place.",
    purpose:
      "Traffic combines visitor, uptime, CDN, and delivery signals so you can see whether people are reaching the site and whether the site is holding up while they do.",
    lookFirst: [
      "Start with the period selector and make sure you are comparing the right window.",
      "Check visitors and pageviews for demand, then uptime and response time for reliability.",
      "Use top pages and sources to spot which pages or channels actually matter to the business.",
    ],
    useWell: [
      "Read traffic changes alongside deploys and alerts before assuming a marketing or product cause.",
      "For operational context, stay here. For search visibility and indexing problems, go to Search & SEO.",
      "If there is no data, connect Plausible, GA4, Cloudflare, or uptime monitoring in Integrations.",
    ],
    takeAction: [
      "Traffic drops sharply without a known seasonal, campaign, or deployment reason.",
      "Uptime, response time, or CDN behavior worsens while traffic is normal.",
      "A key page stops receiving visits or starts behaving differently from the rest of the site.",
    ],
    proTip:
      "Do not optimize for the average page first. Look for the pages that carry revenue, signups, leads, or trust, then protect those paths.",
  },
  issues: {
    title: "Issues Guide",
    subtitle: "The work list for fixes, regressions, verification, and proof.",
    purpose:
      "Issues is where SiteCMD turns scan output into work you can actually finish. It combines live-site issues, code issues, paused work, regressions, and history so you are not bouncing between reports.",
    lookFirst: [
      "Start on the Issues tab and handle critical, high, or regressed items before routine cleanup.",
      "Use Pages when you care about a specific money page, landing page, checkout path, or support page.",
      "Use History when you need to prove whether things improved after a fix or got worse after a deploy.",
    ],
    useWell: [
      "Open the dossier for an issue when you need impact, fix steps, likely files, and verification guidance.",
      "After fixing something meaningful, run a fresh scan when you need proof that it cleared.",
      "Dismiss only when the issue is intentionally accepted for this site, not because it is annoying.",
    ],
    takeAction: [
      "An item blocks launch, affects a key page, or keeps coming back after being fixed.",
      "A new issue appears after a deploy, dependency update, or content change.",
      "The same pattern appears across multiple pages and should be fixed at the template or component level.",
    ],
    proTip:
      "Think of Issues as the working room. The score matters, but the next best action matters more.",
  },
  alerts: {
    title: "Alerts Guide",
    subtitle: "Important changes that deserve attention before normal task work.",
    purpose:
      "Alerts is for changes that are urgent enough to break normal flow: outages, regressions, threats, integration failures, and unusual drops from connected services.",
    lookFirst: [
      "Check unread alerts first, then severity and source.",
      "Read the alert title and source before opening details. It usually tells you whether this is uptime, traffic, search, deploy, or security.",
      "Use source coverage to see whether important monitoring systems are actually connected.",
    ],
    useWell: [
      "Mark alerts read when someone has viewed them; dismiss only when they are resolved or not relevant.",
      "Use Issues for routine work. Keep Alerts reserved for things that changed enough to need attention now.",
      "If alerts seem quiet, confirm the relevant integrations and built-in checks are connected.",
    ],
    takeAction: [
      "The site is down, slow, blocked, or suddenly losing traffic.",
      "A scan, deploy, or connected service reports a meaningful regression.",
      "An alert repeats after you thought the cause was fixed.",
    ],
    proTip:
      "A good alert page should stay boring most days. When it is not boring, work from source and severity before guessing.",
  },
  deploys: {
    title: "Deployments Guide",
    subtitle: "A release log for understanding what changed and what broke afterward.",
    purpose:
      "Deployments helps you answer: what changed recently, did it ship cleanly, and did the site get worse afterward? It links commits, CI, deploy events, and scans when those signals are available.",
    lookFirst: [
      "Start with the most recent deploy or commit and check whether CI passed.",
      "Look for scan regressions that happened after a deploy.",
      "Check pending commits since the last scan if the local project has moved ahead of the last verified site state.",
    ],
    useWell: [
      "Scan after meaningful deploys so the app can connect changes to outcomes.",
      "When a problem appears, start here to find the likely time window or commit range.",
      "Connect GitHub and keep the project folder linked for richer release context.",
    ],
    takeAction: [
      "CI fails, a deploy regresses the site, or a recent commit aligns with a new issue.",
      "A production change shipped without a follow-up scan.",
      "You need to decide whether to fix forward, revert, or investigate a specific release.",
    ],
    proTip:
      "When something breaks, do not start with every possible cause. Start with the most recent deploy window.",
  },
  updates: {
    title: "Dependency Updates Guide",
    subtitle: "Dependency maintenance separated from launch-risk upgrades.",
    purpose:
      "Package Updates separates normal maintenance from updates that carry launch, security, or compatibility risk. It is designed to help you upgrade intentionally instead of letting dependency work pile up.",
    lookFirst: [
      "Check security updates first, especially critical or high vulnerabilities.",
      "Scan major version updates for likely breaking changes before applying them casually.",
      "Use the update details to decide whether a package is routine maintenance or launch-risk work.",
    ],
    useWell: [
      "Batch low-risk patch and minor updates when tests are healthy.",
      "Handle major framework, build tool, database, auth, and payment updates as separate launch-risk decisions.",
      "After applying updates, run the relevant build, tests, and SiteCMD verification path.",
    ],
    takeAction: [
      "A package update fixes a known vulnerability.",
      "A pinned or old core dependency blocks launch confidence.",
      "An update was applied but has not been verified against the real app.",
    ],
    proTip:
      "The goal is not newest-at-all-costs. The goal is knowing which packages are safe routine work and which ones need a deliberate release check.",
  },
  events: {
    title: "Activity Guide",
    subtitle: "The project timeline for answering what happened and when.",
    purpose:
      "Activity Timeline is the memory of the site: scans, deploys, alerts, integration events, verification work, and notable follow-ups in one chronological view.",
    lookFirst: [
      "Start with the time range around the change you care about.",
      "Look for clusters: a deploy, then a scan regression, then an alert often tells a useful story.",
      "Use filters when you only need deploys, scans, alerts, or verification events.",
    ],
    useWell: [
      "Use Activity when someone asks, 'what changed?' or 'when did this start?'",
      "Pair timeline evidence with Issues or Deployments to move from history to action.",
      "Keep integrations connected so the timeline records business and technical events together.",
    ],
    takeAction: [
      "A customer, teammate, or stakeholder asks for proof of what happened.",
      "A regression needs a likely cause window.",
      "Repeated events show a process problem, not just a one-off issue.",
    ],
    proTip:
      "A timeline is strongest when it is used for decisions, not archaeology. Find the event that changes the next step.",
  },
  "search-console": {
    title: "Search & SEO Guide",
    subtitle: "Search visibility, indexing, and crawl health for important pages.",
    purpose:
      "Search & SEO focuses on whether search engines can discover, understand, and keep sending traffic to the site. It combines search visibility with technical crawl and indexing signals.",
    lookFirst: [
      "Start with search regressions or indexing issues before general optimization ideas.",
      "Check the pages that matter most for leads, sales, brand trust, or support.",
      "Look for technical blockers like robots, sitemap, redirects, canonical tags, or crawl errors.",
    ],
    useWell: [
      "For search visibility, stay here. For all visitor sources, go to Traffic & Uptime.",
      "After fixing SEO or crawl issues, verify with a scan and the connected search source when available.",
      "Treat content, technical SEO, and performance together when a key page is losing visibility.",
    ],
    takeAction: [
      "A key page drops in impressions, clicks, indexing, or crawl health.",
      "Search engines are blocked from important pages.",
      "Metadata, structured data, redirects, or canonical signals create confusion.",
    ],
    proTip:
      "SEO work should start with important pages and real visibility signals, not generic checklists.",
  },
  integrations: {
    title: "Integrations Guide",
    subtitle: "The outside services SiteCMD can listen to and reason about.",
    purpose:
      "Integrations are the inputs that turn SiteCMD from a scanner into an operating console. The more relevant sources you connect, the better the app can explain what changed and what matters.",
    lookFirst: [
      "Connect the services you already trust for traffic, uptime, search, deploys, and security.",
      "Check connection health before assuming a page has no data.",
      "Prioritize integrations that match how the site makes money or serves customers.",
    ],
    useWell: [
      "Connect one source at a time and confirm data appears where expected.",
      "Use Settings for workspace or app behavior; use Integrations for external service signals.",
      "Review stale or failing integrations when Dashboard, Alerts, or Traffic look incomplete.",
    ],
    takeAction: [
      "A page says it has no data but you know the service exists.",
      "An integration is stale, disabled, or failing authentication.",
      "You are preparing for launch and need proof from uptime, search, traffic, or deploy systems.",
    ],
    proTip:
      "Do not connect everything just because it exists. Connect the sources that change decisions.",
  },
  sites: {
    title: "Overview Guide",
    subtitle: "The portfolio view for deciding which site needs attention first.",
    purpose:
      "Overview is the portfolio view. It helps owners and operators see which sites are healthy, which ones need attention, and where to jump next.",
    lookFirst: [
      "Check total active issues and average scores to understand the overall workload.",
      "Find the site with the strongest critical, launch, or regression signal.",
      "Use the current-site highlight to confirm which site you are about to open.",
    ],
    useWell: [
      "Use Overview when you manage multiple client sites, products, or environments.",
      "Open the specific site before making changes so scans, settings, and reports apply to the right project.",
      "Add a site when a new property needs its own history, integrations, and launch workflow.",
    ],
    takeAction: [
      "One site has critical issues while the rest are stable.",
      "A site has not been scanned or checked recently.",
      "You need to compare where your limited maintenance time will have the most impact.",
    ],
    proTip:
      "This is a workload map. Pick the site that needs attention, then use the specialized pages to do the work.",
  },
  settings: {
    title: "Project Settings Guide",
    subtitle: "The project and app controls that make the rest of SiteCMD trustworthy.",
    purpose:
      "Settings holds the project, account, environment, license, and app preferences that affect how SiteCMD behaves. It is where you make the workspace match the real site and team.",
    lookFirst: [
      "Confirm the project name, environment URLs, and local folder are correct.",
      "Check account, license, and feature access when a capability seems unavailable.",
      "Review app preferences that affect notifications, monitoring, or desktop behavior.",
    ],
    useWell: [
      "Keep environment URLs accurate so scans and connected signals point at the right place.",
      "Use Integrations for external service setup; use Settings for workspace and app-level choices.",
      "Before deleting or changing project details, make sure you are on the intended project.",
    ],
    takeAction: [
      "A scan or report is using the wrong site URL.",
      "The local folder moved or the app cannot find project files.",
      "Feature access, billing, or desktop behavior does not match expectations.",
    ],
    proTip:
      "Settings should be boring because it is correct. If the rest of the app feels off, verify the project basics here.",
  },
  reports: {
    title: "Reports Guide",
    subtitle: "Shareable status, evidence, and next steps for people outside the app.",
    purpose:
      "Reports packages SiteCMD issues and connected signals into a format you can send to a client, stakeholder, teammate, or future you.",
    lookFirst: [
      "Choose the report scope based on the audience: launch readiness, current health, security, or recent progress.",
      "Check that the latest scan and connected service data are fresh before generating.",
      "Review whether the report should explain risk, progress, or next actions.",
    ],
    useWell: [
      "Generate reports after verification, not before, when you need proof of improvement.",
      "Use plain-language summaries for business readers and detailed issues for technical handoff.",
      "Refresh scans or integrations first if the report would otherwise describe stale conditions.",
    ],
    takeAction: [
      "A client or stakeholder needs a clear status update.",
      "You need launch evidence, maintenance proof, or a before-and-after record.",
      "The report data is stale and should be refreshed before sharing.",
    ],
    proTip:
      "A good report should answer: what is the state, why does it matter, and what happens next?",
  },
  score: {
    title: "SiteCMD Score Guide",
    subtitle: "One number for the whole site, and exactly how it is computed.",
    purpose:
      "The SiteCMD Score is a 0 to 100 reading of the live site and its code together. It starts at 100 and loses points for every open issue, weighted by severity and by how sure SiteCMD is that the finding is real, so one critical problem costs more than a handful of low ones.",
    lookFirst: [
      "Open the breakdown under the ring to see the starting 100 and the points each severity band took away.",
      "A capped score means a confirmed-exploitable critical issue was found; no amount of low-severity cleanup lifts it until that is fixed.",
      "Watch the checked time. A score from last week describes last week's site.",
    ],
    useWell: [
      "Fix the band that took the most points first; that is where the score moves fastest.",
      "Rescan after a fix so the score reflects the site as it is now, not as it was.",
      "Ignore only issues you have genuinely accepted. Ignoring to raise the number hides risk from you, not from visitors.",
    ],
    takeAction: [
      "The score drops after a deploy, a content change, or a dependency update.",
      "The breakdown shows critical or high points and you did not expect either.",
      "The score is capped and the confirmed-exploitable issue is still open.",
    ],
    proTip:
      "Treat the Excellent band as healthy and anything below Good as needing a plan this week. The bands are the same in every report, dashboard, and scan summary.",
  },
};

export function PageGuideButton({ page, className }: { page: PageGuideKey; className?: string }) {
  const guide = PAGE_GUIDES[page];
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);

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
        aria-label={`Open ${guide.title}`}
        title={guide.title}>
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
  const subtitleId = useId();

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
      describedBy={subtitleId}
      onClose={requestClose}
      restoreFocusTo={triggerRef}
      className={cn(
        "page-guide-panel",
        visible ? "page-guide-panel-visible" : "page-guide-panel-hidden",
      )}>
      <div className="page-guide-header">
        <div className="flex-fill">
          <p className="details-eyebrow">Page Guide</p>
          <h2 id={titleId} className="details-title">
            {guide.title}
          </h2>
          <p id={subtitleId} className="details-subtitle">
            {guide.subtitle}
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
        <GuideSection index={1} label="What this page is for" tone="attention">
          <p className="page-guide-paragraph">{guide.purpose}</p>
        </GuideSection>
        <GuideSection index={2} label="Look at first" tone="action">
          <GuideList items={guide.lookFirst} />
        </GuideSection>
        <GuideSection index={3} label="Use it well" tone="supporting">
          <GuideList items={guide.useWell} />
        </GuideSection>
        <GuideSection index={4} label="When to take action" tone="verify">
          <GuideList items={guide.takeAction} />
        </GuideSection>
        <section className="page-guide-tip">
          <p className="details-section-label">Operator tip</p>
          <p>{guide.proTip}</p>
        </section>
      </div>
    </Dialog>
  );
}

function GuideSection({
  index,
  label,
  tone,
  children,
}: {
  index: number;
  label: string;
  tone: "attention" | "action" | "supporting" | "verify";
  children: ReactNode;
}) {
  return (
    <section
      className={cn("dossier-numbered-section page-guide-section", `dossier-section-tone-${tone}`)}>
      <div className="dossier-numbered-header">
        <span className="dossier-numbered-index">{String(index).padStart(2, "0")}</span>
        <span className="dossier-numbered-label">{label}</span>
      </div>
      <div className="dossier-numbered-body">{children}</div>
    </section>
  );
}

function GuideList({ items }: { items: string[] }) {
  return (
    <ul className="page-guide-list">
      {items.map((item) => (
        <li key={item}>{item}</li>
      ))}
    </ul>
  );
}
