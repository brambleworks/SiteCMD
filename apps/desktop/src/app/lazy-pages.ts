import { lazy } from "react";

// Lazy-loaded page components - only parsed/loaded when user navigates to them.
export const Dashboard = lazy(() =>
  import("@/components/dashboard/Dashboard").then((m) => ({ default: m.Dashboard })),
);

export const AnalyticsPage = lazy(() =>
  import("@/components/dashboard/AnalyticsPage").then((m) => ({ default: m.AnalyticsPage })),
);

export const DeploysPage = lazy(() =>
  import("@/components/dashboard/DeploysPage").then((m) => ({ default: m.DeploysPage })),
);

export const EventsPage = lazy(() =>
  import("@/components/events/EventsPage").then((m) => ({ default: m.EventsPage })),
);

export const SettingsPage = lazy(() =>
  import("@/components/settings/SettingsPage").then((m) => ({ default: m.SettingsPage })),
);

export const SearchConsolePage = lazy(() =>
  import("@/components/dashboard/SearchConsolePage").then((m) => ({
    default: m.SearchConsolePage,
  })),
);

export const UpdatesPage = lazy(() =>
  import("@/components/dashboard/UpdatesPage").then((m) => ({ default: m.UpdatesPage })),
);

export const IntegrationsPage = lazy(() =>
  import("@/components/integrations/IntegrationsPage").then((m) => ({
    default: m.IntegrationsPage,
  })),
);

export const SitesOverview = lazy(() =>
  import("@/components/sites/SitesOverview").then((m) => ({ default: m.SitesOverview })),
);

export const ReportsPage = lazy(() =>
  import("@/components/reports/ReportsPage").then((m) => ({ default: m.ReportsPage })),
);

export const IssuesPage = lazy(() =>
  import("@/pages/IssuesPage").then((m) => ({ default: m.IssuesPage })),
);

export const AlertsPage = lazy(() =>
  import("@/pages/AlertsPage").then((m) => ({ default: m.AlertsPage })),
);

// Lazy-loaded scan/project overlays - only needed when viewing/running setup flows.
export const ScanOverlay = lazy(() =>
  import("@/components/scan/ScanOverlay").then((m) => ({ default: m.ScanOverlay })),
);

export const ScanConfigOverlay = lazy(() =>
  import("@/components/scan/ScanConfigOverlay").then((m) => ({ default: m.ScanConfigOverlay })),
);

export const AddProjectForm = lazy(() =>
  import("@/components/project/AddProjectForm").then((m) => ({ default: m.AddProjectForm })),
);

// Keep post-scan onboarding out of the initial bundle.
export const FirstRunWalkthrough = lazy(() =>
  import("@/app/FirstRunWalkthrough").then((m) => ({ default: m.FirstRunWalkthrough })),
);
