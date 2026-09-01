import { useState, useEffect, type ReactNode } from "react";
import {
  LayoutDashboard,
  GitBranch,
  BarChart3,
  Layers,
  CalendarDays,
  Globe,
  RefreshCw,
  FileText,
  ListChecks,
  Bell,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Plug,
} from "lucide-react";
import { useNavBadges, useNavIntegrations } from "@/lib/nav-badges";
import { isNavPageConnected } from "@/lib/nav-integrations";
import { useIssuesBadge } from "@/lib/issues-badge";
import { useRenderSanityCheck } from "@/lib/render-sanity";
import { useTheme } from "@/hooks/useTheme";
import { Button } from "@/components/ui/button";
import { type NavPage } from "@/components/layout/nav-page";

// Re-exported so existing `import { NavPage } from ".../NavSidebar"` sites keep
// working; the canonical home is./nav-page.
export type { NavPage, NavTarget } from "@/components/layout/nav-page";

interface NavSidebarProps {
  activePage: NavPage;
  activeProjectId?: number;
  projectCount: number;
  /** Whether the active project has a linked local folder (gives Deploys git
   *  history even with no GitHub connected). */
  hasLinkedFolder?: boolean;
  onNavigate: (page: NavPage) => void;
  alertsBadge?: number | null;
  alertsCriticalBadge?: number | null;
}

interface NavItem {
  page: NavPage;
  label: string;
  icon: typeof LayoutDashboard;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

// Manage and History remain available without integrations.
const MANAGE_ITEMS: NavItem[] = [
  { page: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { page: "issues", label: "Issues", icon: ListChecks },
  { page: "alerts", label: "Alerts", icon: Bell },
  { page: "updates", label: "Updates", icon: RefreshCw },
  { page: "integrations", label: "Integrations", icon: Plug },
];

const MONITOR_ITEMS: NavItem[] = [
  { page: "analytics", label: "Traffic", icon: BarChart3 },
  { page: "search-console", label: "Search & SEO", icon: Globe },
];

// Deploys appears for projects with local Git history or a GitHub connection.
const DEPLOYS_ITEM: NavItem = { page: "deploys", label: "Deploys", icon: GitBranch };

// Keep an active Deploys page visible even after its Git source disconnects.
function buildHistoryItems(
  enabledIntegrations: ReadonlySet<string>,
  activePage: NavPage,
  hasLinkedFolder: boolean,
): NavItem[] {
  const showDeploys =
    hasLinkedFolder ||
    isNavPageConnected("deploys", enabledIntegrations) ||
    activePage === "deploys";
  return [
    { page: "events", label: "Activity", icon: CalendarDays },
    ...(showDeploys ? [DEPLOYS_ITEM] : []),
    { page: "reports", label: "Reports", icon: FileText },
  ];
}

const STORAGE_KEY = "sitecmd:nav-collapsed";

export function NavSidebar({
  activePage,
  activeProjectId,
  projectCount,
  hasLinkedFolder = false,
  onNavigate,
  alertsBadge,
  alertsCriticalBadge,
}: NavSidebarProps) {
  useRenderSanityCheck("NavSidebar");
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem(STORAGE_KEY) === "1");
  const { updates: updatesBadge } = useNavBadges(activeProjectId);
  const issuesBadge = useIssuesBadge(activeProjectId);
  const enabledIntegrations = useNavIntegrations(activeProjectId);
  const { resolved } = useTheme();

  const historyItems = buildHistoryItems(enabledIntegrations, activePage, hasLinkedFolder);
  const groups: NavGroup[] = [
    { label: "Manage", items: MANAGE_ITEMS },
    { label: "Monitor", items: MONITOR_ITEMS },
    { label: "History", items: historyItems },
  ];

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, collapsed ? "1" : "0");
    document.documentElement.style.setProperty(
      "--sitecmd-sidebar-width",
      collapsed ? "52px" : "13rem",
    );
  }, [collapsed]);

  return (
    <nav className={`nav-sidebar ${collapsed ? "nav-sidebar-collapsed" : ""}`}>
      <div className={`nav-logo-row ${collapsed ? "nav-logo-row-collapsed" : ""}`}>
        <img
          src={
            collapsed
              ? "/favicon.svg"
              : resolved === "dark"
                ? "/images/logo.png"
                : "/images/logo-dark.png"
          }
          alt="SiteCMD"
          className="nav-logo"
          width={collapsed ? 32 : 285}
          height={collapsed ? 32 : 76}
        />
      </div>

      {projectCount > 1 && (
        <>
          <div className="nav-group">
            <Button
              unstyled
              onClick={() => onNavigate("sites")}
              className={`nav-item ${activePage === "sites" ? "nav-item-active" : "nav-item-inactive"}`}
              aria-current={activePage === "sites" ? "page" : undefined}
              title={collapsed ? "Overview" : undefined}>
              <Layers className="nav-icon nav-icon-overview" />
              {!collapsed && "Overview"}
            </Button>
          </div>
          <div className="nav-divider" />
        </>
      )}

      <div className="nav-items">
        {groups.map((group, index) => (
          <div key={group.label} className={`nav-group ${index === 0 ? "" : "nav-group--divided"}`}>
            {!collapsed && (
              <div className="nav-group-label-wrap">
                <span className="nav-group-label">{group.label}</span>
              </div>
            )}
            {collapsed ? <div className="nav-collapsed-spacer" /> : null}
            <div className="stack-hair">
              {group.items.map(({ page, label, icon: Icon }) => {
                // Narrow to the badge value (not a stored boolean) so the JSX
                // below reads it without non-null assertions.
                const updatesBadgeToShow =
                  page === "updates" && updatesBadge && updatesBadge.total > 0
                    ? updatesBadge
                    : null;
                const issuesBadgeToShow =
                  page === "issues" && issuesBadge && issuesBadge.total > 0 ? issuesBadge : null;
                const showAlertsBadge = page === "alerts" && alertsBadge != null && alertsBadge > 0;
                const alertsHasCritical = alertsCriticalBadge != null && alertsCriticalBadge > 0;
                return (
                  <Button
                    unstyled
                    key={page}
                    onClick={() => onNavigate(page)}
                    className={`nav-item ${
                      activePage === page ? "nav-item-active" : "nav-item-inactive"
                    }`}
                    aria-current={activePage === page ? "page" : undefined}
                    title={collapsed ? label : undefined}>
                    <Icon className="nav-icon" />
                    {!collapsed && label}
                    {!collapsed && updatesBadgeToShow && (
                      <span
                        className={`nav-count-badge ${
                          updatesBadgeToShow.critical > 0
                            ? "nav-count-critical"
                            : "nav-count-primary"
                        }`}
                        title={
                          updatesBadgeToShow.critical > 0
                            ? `${updatesBadgeToShow.critical} critical security update${updatesBadgeToShow.critical === 1 ? "" : "s"} out of ${updatesBadgeToShow.total} total package update${updatesBadgeToShow.total === 1 ? "" : "s"}`
                            : `${updatesBadgeToShow.total} package update${updatesBadgeToShow.total === 1 ? "" : "s"}`
                        }>
                        {updatesBadgeToShow.total}
                      </span>
                    )}
                    {!collapsed && issuesBadgeToShow && (
                      <span
                        className={`nav-count-badge ${
                          issuesBadgeToShow.critical > 0
                            ? "nav-count-critical"
                            : "nav-count-primary"
                        }`}
                        title={`${issuesBadgeToShow.total} active issue${issuesBadgeToShow.total === 1 ? "" : "s"}`}>
                        {issuesBadgeToShow.total}
                      </span>
                    )}
                    {!collapsed && showAlertsBadge && (
                      <span
                        className={`nav-count-badge ${
                          alertsHasCritical ? "nav-count-critical" : "nav-count-primary"
                        }`}
                        title={
                          alertsHasCritical
                            ? `${alertsCriticalBadge} critical unread alert${alertsCriticalBadge === 1 ? "" : "s"} out of ${alertsBadge} unread alert${alertsBadge === 1 ? "" : "s"}`
                            : `${alertsBadge} unread alert${alertsBadge === 1 ? "" : "s"}`
                        }>
                        {alertsBadge}
                      </span>
                    )}
                  </Button>
                );
              })}
            </div>
          </div>
        ))}
      </div>

      <SidebarUtilityBar
        activePage={activePage}
        collapsed={collapsed}
        onNavigate={onNavigate}
        onToggleCollapse={() => setCollapsed((c) => !c)}
      />
    </nav>
  );
}

function SidebarUtilityBar({
  activePage,
  collapsed,
  onNavigate,
  onToggleCollapse,
}: {
  activePage: NavPage;
  collapsed: boolean;
  onNavigate: (page: NavPage) => void;
  onToggleCollapse: () => void;
}) {
  const [hoverLabel, setHoverLabel] = useState<string | null>(null);
  const collapseLabel = collapsed ? "Expand sidebar" : "Collapse sidebar";
  const setActiveLabel = (label: string) => setHoverLabel(label);
  const clearActiveLabel = () => setHoverLabel(null);

  return (
    <div className={`nav-utility-bar ${collapsed ? "nav-utility-bar-collapsed" : ""}`}>
      {!collapsed ? (
        <span
          className={`nav-utility-tooltip ${hoverLabel ? "nav-utility-tooltip-visible" : ""}`}
          aria-hidden="true">
          {hoverLabel}
        </span>
      ) : null}
      <SidebarUtilityButton
        label="Settings"
        onClick={() => onNavigate("settings")}
        active={activePage === "settings"}
        onLabelShow={setActiveLabel}
        onLabelHide={clearActiveLabel}>
        <Settings className="nav-utility-icon" />
      </SidebarUtilityButton>
      <SidebarUtilityButton
        label={collapseLabel}
        onClick={onToggleCollapse}
        onLabelShow={setActiveLabel}
        onLabelHide={clearActiveLabel}>
        {collapsed ? (
          <PanelLeftOpen className="nav-utility-icon" />
        ) : (
          <PanelLeftClose className="nav-utility-icon" />
        )}
      </SidebarUtilityButton>
    </div>
  );
}

function SidebarUtilityButton({
  label,
  active,
  onClick,
  onLabelShow,
  onLabelHide,
  children,
}: {
  label: string;
  active?: boolean;
  onClick: () => void;
  onLabelShow: (label: string) => void;
  onLabelHide: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      onClick={onClick}
      onMouseEnter={() => onLabelShow(label)}
      onMouseLeave={onLabelHide}
      onFocus={() => onLabelShow(label)}
      onBlur={onLabelHide}
      className={`nav-utility-button ${active ? "nav-utility-button-active" : ""}`}
      aria-label={label}
      aria-current={active ? "page" : undefined}>
      {children}
    </Button>
  );
}
