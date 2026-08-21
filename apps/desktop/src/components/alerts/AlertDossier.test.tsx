import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AlertDossier } from "./AlertDossier";
import { parseAlertDetailRecord } from "./alert-detail-model";
import type { AlertRow } from "@/lib/types";

const baseAlert: AlertRow = {
  id: 1,
  projectId: 7,
  envUrl: "https://example.com",
  source: "github",
  alertId: "alert-1",
  severity: "warn",
  title: "Build failed",
  description: "The latest deployment failed before it reached production.",
  detailJson: null,
  occurredAt: 1_700_000_000_000,
  firstSeenAt: 1_700_000_000_000,
  lastSeenAt: 1_700_000_000_000,
  viewedAt: null,
  dismissedAt: null,
};

describe("parseAlertDetailRecord", () => {
  it("keeps valid alert metadata records", () => {
    expect(parseAlertDetailRecord('{"destination":"deploys","build_id":42}')).toEqual({
      destination: "deploys",
      build_id: 42,
    });
  });

  it.each([null, "", "not json", "[]", '"deploys"', "42"])(
    "drops malformed or non-record alert metadata: %s",
    (detailJson) => {
      expect(parseAlertDetailRecord(detailJson)).toEqual({});
    },
  );
});

describe("AlertDossier", () => {
  it("does not render metadata context details", () => {
    render(
      <AlertDossier
        alert={{
          ...baseAlert,
          detailJson: JSON.stringify({
            alert_type: "security_update",
            destination: "updates",
            package: "lodash",
            latest_version: "4.17.21",
          }),
        }}
        onMarkViewed={vi.fn()}
        onMarkUnread={vi.fn()}
        onDismiss={vi.fn()}
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.getByText("Build failed")).toBeInTheDocument();
    expect(screen.queryByText("Useful Context")).not.toBeInTheDocument();
    expect(screen.queryByText("Package")).not.toBeInTheDocument();
    expect(screen.queryByText("lodash")).not.toBeInTheDocument();
  });

  it("renders alert actions as labeled buttons and colors only the severity in the eyebrow", () => {
    const onMarkViewed = vi.fn();
    render(
      <AlertDossier
        alert={baseAlert}
        onMarkViewed={onMarkViewed}
        onMarkUnread={vi.fn()}
        onDismiss={vi.fn()}
        onNavigate={vi.fn()}
      />,
    );

    // Only the severity keeps its tone color; the source follows it neutrally.
    expect(screen.getByText("Warning")).toHaveClass("text-severity-medium");
    expect(screen.queryByText("Alert details")).not.toBeInTheDocument();
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();

    const markRead = screen.getByRole("button", { name: "Mark read" });
    fireEvent.click(markRead);
    expect(onMarkViewed).toHaveBeenCalledTimes(1);
  });

  it("shows alert-type-specific recommended action context", () => {
    render(
      <AlertDossier
        alert={{
          ...baseAlert,
          source: "updates",
          detailJson: JSON.stringify({ alert_type: "security_update", destination: "updates" }),
        }}
        onMarkViewed={vi.fn()}
        onMarkUnread={vi.fn()}
        onDismiss={vi.fn()}
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.getByText("Recommended Action")).toBeInTheDocument();
    expect(screen.getByText(/Update the affected package/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Updates" })).toBeInTheDocument();
  });
});

describe("AlertDossier deploy regression blame", () => {
  const deployRegressionDetail = {
    alert_type: "deploy_regression",
    scan_kind: "web",
    scan_id: 12,
    regression_id: 5,
    previous_score: 92,
    current_score: 84,
    score_drop: 8,
    new_issues: [{ check_id: "missing_csp_header", title: "Missing CSP header" }],
    fixed_count: 1,
    commit_from: "aaa1111111",
    commit_to: "bbb2222222",
    commit_count: 3,
    commits: [
      {
        hash: "bbb2222222",
        short_hash: "bbb2222",
        message: "Rework response header middleware",
        author: "Kyle",
        date: "2026-06-08",
      },
    ],
  };

  function renderDossier(detail: Record<string, unknown>, onNavigate = vi.fn()) {
    render(
      <AlertDossier
        alert={{ ...baseAlert, detailJson: JSON.stringify(detail) }}
        onMarkViewed={vi.fn()}
        onMarkUnread={vi.fn()}
        onDismiss={vi.fn()}
        onNavigate={onNavigate}
      />,
    );
    return onNavigate;
  }

  it("renders the blame section for a deploy-regression alert", () => {
    renderDossier(deployRegressionDetail);

    expect(screen.getByText("What Your Deploy Changed")).toBeInTheDocument();
    expect(screen.getByText("aaa1111..bbb2222")).toBeInTheDocument();
    expect(screen.getByText("Missing CSP header")).toBeInTheDocument();
    expect(screen.getByText(/Also fixed 1 issue/)).toBeInTheDocument();
  });

  it("says which findings were not attributed to the commits", () => {
    renderDossier({
      ...deployRegressionDetail,
      detector_changed_count: 2,
      engine_release: "1.5.4",
    });

    expect(
      screen.getByText(
        /2 other findings come from checks that changed in SiteCMD 1\.5\.4, so they are not attributed to these commits\./,
      ),
    ).toBeInTheDocument();
  });

  it("says nothing about attribution when nothing was held back", () => {
    renderDossier(deployRegressionDetail);

    expect(screen.queryByText(/not attributed to these commits/)).not.toBeInTheDocument();
  });

  it("navigates to issues when an introduced issue is clicked", () => {
    const onNavigate = renderDossier(deployRegressionDetail);

    fireEvent.click(screen.getByRole("button", { name: "Missing CSP header" }));

    expect(onNavigate).toHaveBeenCalledWith("issues");
  });

  it("uses singular copy for a one-point drop", () => {
    renderDossier({
      ...deployRegressionDetail,
      previous_score: 85,
      current_score: 84,
      score_drop: 1,
    });

    expect(screen.getByText(/\(down 1 point\)/)).toBeInTheDocument();
  });

  it("expands and collapses the commit list beyond the first three commits", () => {
    renderDossier({
      ...deployRegressionDetail,
      commit_count: 9,
      commits: [
        {
          hash: "ccc3333333",
          short_hash: "ccc3333",
          message: "Tighten CSP directives",
          author: "Kyle",
          date: "2026-06-08",
        },
        {
          hash: "ddd4444444",
          short_hash: "ddd4444",
          message: "Add header regression test",
          author: "Kyle",
          date: "2026-06-08",
        },
        {
          hash: "eee5555555",
          short_hash: "eee5555",
          message: "Rename middleware module",
          author: "Kyle",
          date: "2026-06-07",
        },
        {
          hash: "fff6666666",
          short_hash: "fff6666",
          message: "Bump framework patch version",
          author: "Kyle",
          date: "2026-06-07",
        },
      ],
    });

    expect(screen.getByText("Tighten CSP directives")).toBeInTheDocument();
    expect(screen.getByText("Add header regression test")).toBeInTheDocument();
    expect(screen.getByText("Rename middleware module")).toBeInTheDocument();
    expect(screen.queryByText("Bump framework patch version")).not.toBeInTheDocument();
    expect(screen.queryByText(/commits shown/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Show 1 more commit" }));

    expect(screen.getByText("Bump framework patch version")).toBeInTheDocument();
    expect(screen.getByText("Newest 4 of 9 commits shown.")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Show fewer commits" }));

    expect(screen.queryByText("Bump framework patch version")).not.toBeInTheDocument();
    expect(screen.getByText("Tighten CSP directives")).toBeInTheDocument();
    expect(screen.getByText("Add header regression test")).toBeInTheDocument();
    expect(screen.getByText("Rename middleware module")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show 1 more commit" })).toBeInTheDocument();
  });

  it("uses the new-issues headline when blame fired without a score drop", () => {
    renderDossier({
      ...deployRegressionDetail,
      score_drop: -3,
      previous_score: 80,
      current_score: 83,
    });

    expect(screen.getByText(/even though the score held/)).toBeInTheDocument();
    expect(screen.queryByText(/down -3/)).not.toBeInTheDocument();
  });

  it("renders no blame section when the alert carries no regression dossier", () => {
    renderDossier({ alert_type: "web_score_drop", destination: "issues" });

    expect(screen.queryByText("What Your Deploy Changed")).not.toBeInTheDocument();
    expect(screen.queryByText(/Regression blame is a Plus feature/)).not.toBeInTheDocument();
  });
});
