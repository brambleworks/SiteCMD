import { describe, expect, it } from "vitest";

import {
  buildCodeScanCompletionCopy,
  buildMultiScanCompletionCopy,
  buildScheduledScanCompletionCopy,
  buildWebScanCompletionCopy,
  getScheduledScanLabel,
} from "./scan-completion-copy";

describe("scan completion copy", () => {
  it("builds code scan copy with leading domain, trend, and workflow cue", () => {
    const copy = buildCodeScanCompletionCopy({
      score: 78,
      issueCount: 5,
      scoreMessage: "Looking good!",
      host: "example.com",
      leadingDomain: {
        label: "Database",
        shortLabel: "DB",
        count: 3,
      },
      domainTrendLabel: "Database eased by 2",
      workflowCue: {
        label: "2 regressed",
        sentence: "Resume 2 regressed items next.",
      },
    });

    expect(copy.title).toBe("Code Scan Complete - 78/100");
    expect(copy.body).toBe(
      "5 code issues found for example.com. Database leads with 3. Database eased by 2. Looking good! Resume 2 regressed items next.",
    );
    expect(copy.jobLabel).toBe("Code scan");
    expect(copy.jobDetail).toBe("78/100 • 5 issues • DB 3 • Database eased by 2 • 2 regressed");
  });

  it("builds web scan copy with host-aware wording", () => {
    const copy = buildWebScanCompletionCopy({
      score: 91,
      issueCount: 2,
      scoreMessage: "Looking great!",
      host: "example.com",
      workflowCue: {
        label: "1 working",
        sentence: "1 in-progress item is ready to resume.",
      },
    });

    expect(copy.title).toBe("Web Scan Complete - 91/100");
    expect(copy.body).toBe(
      "2 issues found on example.com. Looking great! 1 in-progress item is ready to resume.",
    );
    expect(copy.jobLabel).toBe("Web scan");
    expect(copy.jobDetail).toBe("91/100 • 2 issues • 1 working");
  });

  it("presents exactly one score with no secondary project-score sentence", () => {
    // The caller passes the unified SiteCMD Score as `score`; there is no
    // separate project score to append.
    const copy = buildWebScanCompletionCopy({
      score: 15,
      issueCount: 2,
      scoreMessage: "Needs attention.",
      host: "example.com",
    });

    expect(copy.title).toBe("Web Scan Complete - 15/100");
    expect(copy.body).toBe("2 issues found on example.com. Needs attention.");
    // Negative control: no "Project score N/100." split remains.
    expect(copy.body).not.toMatch(/Project score/);
  });

  it("builds multi-page copy with shared workflow language", () => {
    const copy = buildMultiScanCompletionCopy({
      score: 84,
      pageCount: 7,
      scoreMessage: "Looking good!",
      workflowCue: {
        label: "3 blocked",
        sentence: "3 blocked items need a decision.",
      },
    });

    expect(copy.title).toBe("Multi-page Web Scan Complete - 84/100");
    expect(copy.body).toBe("7 pages scanned. Looking good! 3 blocked items need a decision.");
    expect(copy.jobDetail).toBe("84/100 • 7 pages • 3 blocked");
  });

  it("builds scheduled code scan copy from the same code-domain summary rules", () => {
    const copy = buildScheduledScanCompletionCopy({
      scanType: "code",
      score: 63,
      issueCount: 9,
      host: "example.com",
      scoreMessage: "Some issues found.",
      topDomain: "database",
      topDomainCount: 4,
      domainTrendLabel: "Database grew by 1",
      workflowCue: {
        label: "1 launch blocker",
        sentence: "1 launch blocker is still open.",
      },
    });

    expect(copy.title).toBe("Scheduled Code Scan Complete - 63/100");
    expect(copy.body).toBe(
      "9 code issues found for example.com. Database Analysis leads with 4. Database grew by 1. Some issues found. 1 launch blocker is still open.",
    );
    expect(copy.jobLabel).toBe("Scheduled Code Scan");
    expect(copy.jobDetail).toBe(
      "63/100 • 9 issues • Database 4 • Database grew by 1 • 1 launch blocker",
    );
  });

  it("reports a scheduled full scan as one Full Scan completion, not web or code", () => {
    const copy = buildScheduledScanCompletionCopy({
      scanType: "full",
      score: 74,
      issueCount: 5,
      host: "example.com",
      scoreMessage: "Some issues found.",
      workflowCue: null,
    });

    expect(copy.title).toBe("Scheduled Full Scan Complete - 74/100");
    expect(copy.jobLabel).toBe("Scheduled Full Scan");
  });

  it("reports incomplete scheduled coverage as partial", () => {
    const copy = buildScheduledScanCompletionCopy({
      scanType: "health",
      status: "partial",
      completedPages: 1,
      totalPages: 2,
      score: 61,
      issueCount: 2,
      host: "example.com",
      scoreMessage: "Needs attention.",
    });

    expect(copy.title).toBe("Scheduled Web Scan Partially Complete - 61/100");
    expect(copy.body).toBe("1 of 2 pages scanned. 2 issues found on example.com. Needs attention.");
    expect(copy.jobDetail).toBe("61/100 • 2 issues • 1 of 2 pages");
  });

  it("maps scheduled scan labels consistently", () => {
    expect(getScheduledScanLabel("code")).toBe("Scheduled Code Scan");
    expect(getScheduledScanLabel("full")).toBe("Scheduled Full Scan");
    expect(getScheduledScanLabel("security")).toBe("Scheduled Web Scan");
    expect(getScheduledScanLabel("health")).toBe("Scheduled Web Scan");
    expect(getScheduledScanLabel(undefined)).toBe("Scheduled Web Scan");
  });
});
