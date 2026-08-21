import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import type { PackageUpdate } from "@/lib/types";
import { buildUpdateQueueSummary } from "@/lib/update-summary";
import type { CompactTrendModel } from "./compact-trend-model";
import type { ActionItemsData, DashboardPrimaryActionCard } from "./zones/ActionItemsCard";
import type { NavTarget } from "@/components/layout/nav-page";

interface BuildDashboardActionItemsArgs {
  allUpdates: PackageUpdate[];
  issueSummary: ProjectIssueSummary;
  issuesTrend: CompactTrendModel;
  onNavigate: (page: NavTarget) => void;
  updatesTrend: CompactTrendModel;
}

export function buildDashboardActionItems(args: BuildDashboardActionItemsArgs): ActionItemsData {
  return {
    cards: buildDashboardPrimaryActionCards(args),
  };
}

function buildDashboardPrimaryActionCards({
  allUpdates,
  issueSummary,
  issuesTrend,
  onNavigate,
  updatesTrend,
}: BuildDashboardActionItemsArgs): DashboardPrimaryActionCard[] {
  const updateSummary = buildUpdateQueueSummary(allUpdates);

  return [
    {
      key: "issues",
      label: "Issues",
      value: `${issueSummary.totalCount} Open`,
      detail: formatIssueBreakdown(issueSummary),
      trend: issuesTrend,
      onClick: () => onNavigate("issues"),
    },
    {
      key: "updates",
      label: "Updates",
      value: `${updateSummary.total} Available`,
      detail: `${updateSummary.security} Security · ${updateSummary.major} Major · ${updateSummary.minor} Minor · ${updateSummary.patch} Patch`,
      trend: updatesTrend,
      onClick: () => onNavigate("updates"),
    },
  ];
}

function formatIssueBreakdown(issueSummary: ProjectIssueSummary): string {
  const { critical, high, medium, low } = issueSummary.severityCounts;
  return `${critical} Critical · ${high} High · ${medium} Medium · ${low} Low`;
}
