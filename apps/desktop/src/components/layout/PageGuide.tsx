import { Suspense, lazy, useCallback, useRef, useState } from "react";
import { CircleHelp } from "lucide-react";
import type { NavPage } from "@/components/layout/nav-page";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export type PageGuideKey = NavPage;

/** Titles the header trigger needs before the guide copy is fetched. */
const PAGE_GUIDE_TITLES: Record<PageGuideKey, string> = {
  dashboard: "Site Dashboard",
  analytics: "Traffic & Uptime",
  issues: "Issues",
  alerts: "Alerts",
  deploys: "Deployments",
  updates: "Updates",
  events: "Activity",
  "search-console": "Search & SEO",
  integrations: "Integrations",
  sites: "Overview",
  settings: "Project Settings",
  reports: "Reports",
};

// The guide copy and its dialog are only needed once someone opens the guide.
const PageGuidePanel = lazy(() => import("@/components/layout/PageGuidePanel"));

export function PageGuideButton({ page, className }: { page: PageGuideKey; className?: string }) {
  const title = PAGE_GUIDE_TITLES[page];
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const guideLabel = `${title} guide`;

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
      {open ? (
        <Suspense fallback={null}>
          <PageGuidePanel page={page} title={title} onClose={handleClose} triggerRef={triggerRef} />
        </Suspense>
      ) : null}
    </>
  );
}
