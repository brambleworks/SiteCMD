/* eslint-disable react-refresh/only-export-components -- normalizeTab is exported for tests. */
import { useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  BadgeDollarSign,
  Cloud,
  Database,
  FolderCog,
  MonitorCog,
  ShieldCheck,
  ScanLine,
  Workflow,
} from "lucide-react";
import { AccountSection } from "./AccountSettings";
import { CICDSection } from "./CICDSection";
import { DataSection, DeleteProjectCard, SiteSetupSection } from "./SettingsDataSection";
import { ScanningSection } from "./SettingsScanPrefsSection";
import { GeneralSection } from "./SettingsGeneralSection";
import { SitemapSection } from "./SitemapSection";
import { TelemetrySettingsSection } from "./TelemetrySettingsSection";
import { WebhooksSection } from "./WebhooksSection";
import { ConnectedServiceSection } from "./ConnectedServiceSection";
import { Button } from "@/components/ui/button";
import type { EnvironmentRecord } from "@/hooks/useProject";

export type SettingsTab =
  | "site-setup"
  | "scanning"
  | "automation"
  | "connected"
  | "account"
  | "app-preferences"
  | "privacy-diagnostics"
  | "data";

type SettingsNavGroup = "project" | "workspace";

interface SettingsNavItem {
  id: SettingsTab;
  group: SettingsNavGroup;
  label: string;
  description: string;
  icon: LucideIcon;
  requiresProject?: boolean;
}

const TABS: SettingsNavItem[] = [
  {
    id: "site-setup",
    group: "project",
    label: "Site Setup",
    description: "Project name, linked folder, environment URLs, sitemap pages, and removal.",
    icon: FolderCog,
    requiresProject: true,
  },
  {
    id: "scanning",
    group: "project",
    label: "Scanning",
    description: "Web Scan timeout, scan history retention, and scheduled scans.",
    icon: ScanLine,
    requiresProject: true,
  },
  {
    id: "automation",
    group: "project",
    label: "Automation",
    description: "GitHub Actions scan gates and webhooks for Slack, Discord, or custom endpoints.",
    icon: Workflow,
    requiresProject: true,
  },
  {
    id: "connected",
    group: "project",
    label: "Connected",
    description: "Inspect, sync, transfer, and unlink this production site's connected state.",
    icon: Cloud,
    requiresProject: true,
  },
  {
    id: "account",
    group: "workspace",
    label: "Account & Billing",
    description: "Current plan, license key, and billing.",
    icon: BadgeDollarSign,
  },
  {
    id: "app-preferences",
    group: "workspace",
    label: "App Preferences",
    description: "Theme, desktop behavior, notifications, and updates.",
    icon: MonitorCog,
  },
  {
    id: "privacy-diagnostics",
    group: "workspace",
    label: "Privacy & Diagnostics",
    description: "Opt-in telemetry, crash reports, data controls, and diagnostic logs.",
    icon: ShieldCheck,
  },
  {
    id: "data",
    group: "workspace",
    label: "Data",
    description: "Local database details, backups, and cleanup.",
    icon: Database,
  },
];

const GROUP_LABELS: Record<SettingsNavGroup, { title: string }> = {
  project: {
    title: "This Project",
  },
  workspace: {
    title: "Workspace",
  },
};

interface SettingsPageProps {
  projectId?: number;
  environmentId?: number;
  projectName?: string;
  url?: string;
  siteId?: number;
  framework?: string;
  projectPath?: string | null;
  initialTab?: string;
  projectEnvironments?: EnvironmentRecord[];
  onProjectChanged?: () => void | Promise<unknown>;
  onProjectDeleted?: () => void | Promise<unknown>;
}

export function normalizeTab(tab?: string): SettingsTab {
  switch (tab) {
    // Project aliases map to Site Setup.
    case "site-setup":
    case "project":
    case "project-basics":
    case "pages":
    case "danger-zone":
    case "danger":
      return "site-setup";
    case "scanning":
    case "scan-settings":
    case "scan-defaults":
    case "scan-prefs":
    case "scan-behavior":
    case "schedules":
    case "scheduled-scans":
      return "scanning";
    case "automation":
    case "automations":
    case "cicd":
    case "ci":
    case "ci-cd":
    case "webhooks":
    case "webhook":
      return "automation";
    case "connected":
    case "connected-service":
    case "sync":
      return "connected";
    case "account":
    case "billing":
      return "account";
    case "app-preferences":
    case "general":
    case "appearance":
    case "about":
      return "app-preferences";
    case "privacy-diagnostics":
    case "privacy":
    case "diagnostics":
    case "telemetry":
      return "privacy-diagnostics";
    case "data":
    case "data-support":
    case "support":
    case "backups":
      return "data";
    default:
      return "account";
  }
}

export function SettingsPage({
  projectId,
  environmentId,
  projectName,
  url,
  siteId,
  framework,
  projectPath,
  initialTab,
  projectEnvironments,
  onProjectChanged,
  onProjectDeleted,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>(
    initialTab ? normalizeTab(initialTab) : projectId ? "site-setup" : "account",
  );
  const activeNavItem = TABS.find((tab) => tab.id === activeTab);

  // Sync the active tab when the initialTab prop changes, adjusting state during
  // render instead of via an effect.
  const [renderedInitialTab, setRenderedInitialTab] = useState(initialTab);
  if (renderedInitialTab !== initialTab) {
    setRenderedInitialTab(initialTab);
    if (initialTab) setActiveTab(normalizeTab(initialTab));
  }

  return (
    <div className="page-content">
      <div className="settings-layout">
        <aside className="settings-sidebar" aria-label="Settings sections">
          {(Object.keys(GROUP_LABELS) as SettingsNavGroup[]).map((group) => (
            <SettingsNavSection
              key={group}
              group={group}
              items={TABS.filter((tab) => tab.group === group)}
              activeTab={activeTab}
              hasProject={Boolean(projectId)}
              onSelect={setActiveTab}
            />
          ))}
        </aside>

        <div className="settings-main">
          {activeNavItem ? <SettingsSectionHeader item={activeNavItem} /> : null}
          {activeTab === "account" ? <AccountSection /> : null}
          {activeTab === "scanning" ? (
            <ScanningSection
              projectId={projectId}
              environmentId={environmentId}
              projectPath={projectPath}
            />
          ) : null}
          {activeTab === "app-preferences" ? <GeneralSection /> : null}
          {activeTab === "privacy-diagnostics" ? <TelemetrySettingsSection /> : null}
          {activeTab === "site-setup" ? (
            <>
              <SiteSetupSection
                framework={framework}
                projectId={projectId}
                projectName={projectName}
                projectPath={projectPath}
                projectEnvironments={projectEnvironments}
                onProjectChanged={onProjectChanged}
              />
              <SitemapSection
                siteUrl={url}
                siteId={siteId}
                projectId={projectId}
                framework={framework}
              />
              <DeleteProjectCard
                projectId={projectId}
                projectName={projectName}
                onProjectDeleted={onProjectDeleted}
              />
            </>
          ) : null}
          {activeTab === "automation" ? (
            <>
              <CICDSection projectPath={projectPath} siteUrl={url} />
              <WebhooksSection projectId={projectId} />
            </>
          ) : null}
          {activeTab === "connected" ? (
            <ConnectedServiceSection
              key={`${projectId ?? "none"}|${url ?? "none"}`}
              projectId={projectId}
              environmentScopeKey={url}
            />
          ) : null}
          {activeTab === "data" ? <DataSection view="data" /> : null}
        </div>
      </div>
    </div>
  );
}

function SettingsSectionHeader({ item }: { item: SettingsNavItem }) {
  const Icon = item.icon;
  return (
    <div className="settings-section-header">
      <div className="settings-section-icon">
        <Icon className="icon-md" />
      </div>
      <div className="settings-section-copy">
        <h2 className="settings-section-title">{item.label}</h2>
        <p className="settings-section-description">{item.description}</p>
      </div>
    </div>
  );
}

function SettingsNavSection({
  group,
  items,
  activeTab,
  hasProject,
  onSelect,
}: {
  group: SettingsNavGroup;
  items: SettingsNavItem[];
  activeTab: SettingsTab;
  hasProject: boolean;
  onSelect: (tab: SettingsTab) => void;
}) {
  const groupMeta = GROUP_LABELS[group];

  return (
    <div className="settings-nav-group">
      <div className="settings-nav-group-header">
        <p className="settings-nav-group-title">{groupMeta.title}</p>
      </div>
      <div className="settings-nav-list">
        {items.map((tab) => {
          const isActive = activeTab === tab.id;
          const disabled = Boolean(tab.requiresProject && !hasProject);
          const Icon = tab.icon;
          return (
            <Button
              key={tab.id}
              type="button"
              onClick={() => !disabled && onSelect(tab.id)}
              disabled={disabled}
              variant="ghost"
              size="sm"
              className={`settings-nav-button ${isActive ? "settings-nav-button-active" : ""} ${
                disabled ? "settings-nav-button-disabled" : ""
              }`}
              title={disabled ? "Select a project first" : tab.label}>
              <Icon className="settings-nav-icon" />
              <span className="settings-nav-label">{tab.label}</span>
            </Button>
          );
        })}
      </div>
    </div>
  );
}
