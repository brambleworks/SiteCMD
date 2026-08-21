import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { CompactTrendModel } from "@/components/dashboard/compact-trend-model";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import type { PackageUpdate } from "@/lib/types";
import { buildDashboardActionItems } from "./dashboard-action-items";
import { ActionItemsCard } from "./zones/ActionItemsCard";

const emptyIssueSummary: ProjectIssueSummary = {
  webCount: 0,
  codeCount: 0,
  totalCount: 0,
  criticalCount: 0,
  severityCounts: { critical: 0, high: 0, medium: 0, low: 0 },
};

function packageUpdate(overrides: Partial<PackageUpdate>): PackageUpdate {
  return {
    name: "example",
    currentVersion: "1.0.0",
    latestVersion: "2.0.0",
    ecosystem: "npm",
    updateType: "patch",
    isSecurity: false,
    advisorySeverity: null,
    advisoryUrl: null,
    source: "package.json",
    isDev: false,
    isDeprecated: false,
    deprecationMessage: null,
    currentVersionDeprecated: false,
    isStale: false,
    lastPublished: null,
    workspaceMembers: [],
    ...overrides,
  };
}

function trend(key: string): CompactTrendModel {
  return {
    key,
    label: key,
    currentValue: "0",
    detail: "No trend",
    deltaLabel: "No trend yet",
    tone: "empty",
    series: [0],
  };
}

function buildArgs(overrides: Partial<Parameters<typeof buildDashboardActionItems>[0]> = {}) {
  return {
    allUpdates: [],
    issueSummary: emptyIssueSummary,
    issuesTrend: trend("issues-trend"),
    onNavigate: vi.fn(),
    updatesTrend: trend("updates-trend"),
    ...overrides,
  };
}

describe("buildDashboardActionItems", () => {
  it("renders exactly Issues and Updates as primary action cards", () => {
    const data = buildDashboardActionItems(buildArgs());

    render(<ActionItemsCard items={data} />);

    expect(screen.getByText("Issues")).toBeInTheDocument();
    expect(screen.getByText("Updates")).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(2);
  });

  it("combines security patches and dependency updates into one updates card", () => {
    const onNavigate = vi.fn();
    const data = buildDashboardActionItems(
      buildArgs({
        allUpdates: [
          packageUpdate({
            name: "lodash",
            isSecurity: true,
            advisorySeverity: "critical",
          }),
          packageUpdate({
            name: "vite",
            isSecurity: true,
            advisorySeverity: "high",
          }),
          packageUpdate({
            name: "react",
            updateType: "major",
          }),
          packageUpdate({
            name: "typescript",
            updateType: "minor",
          }),
          packageUpdate({
            name: "eslint",
            updateType: "patch",
          }),
        ],
        onNavigate,
      }),
    );

    render(<ActionItemsCard items={data} />);

    expect(screen.getByText("5 Available")).toBeInTheDocument();
    expect(screen.getByText("2 Security · 1 Major · 1 Minor · 1 Patch")).toBeInTheDocument();
    expect(screen.queryByText("Security Patches")).not.toBeInTheDocument();
    expect(screen.queryByText("Dependency Updates")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("Updates").closest("button")!);
    expect(onNavigate).toHaveBeenCalledWith("updates");
  });
});
