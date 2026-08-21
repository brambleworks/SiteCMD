import { describe, expect, it } from "vitest";

import type { MultiScanResult } from "@/hooks/useScan";
import type { ScanSummary, ScanSessionSummary } from "@/hooks/useHistory";
import type { CodeScanResult, CodeScanSummary, ScanResult } from "@/lib/types";
import { buildProjectIssueSummary } from "@/lib/project-issue-summary";
import { buildScanSummaryModel, buildSkippedScopeNote } from "./scan-summary-model";

function webResult(overrides: Partial<ScanResult> = {}): ScanResult {
  return {
    url: "https://example.com",
    mode: "live",
    scanType: "health",
    overallScore: 72,
    categories: [],
    issues: [
      {
        checkId: "hsts",
        category: "security",
        title: "Missing HSTS",
        description: "No HSTS header.",
        status: "fail",
        severity: "high",
        fixPrompt: null,
        manualFix: null,
        rawData: null,
        confidence: "high",
      },
      {
        checkId: "meta-description",
        category: "seo",
        title: "Missing meta description",
        description: "No description.",
        status: "warn",
        severity: "medium",
        fixPrompt: null,
        manualFix: null,
        rawData: null,
        confidence: "high",
      },
      {
        checkId: "title",
        category: "seo",
        title: "Title exists",
        description: "Looks good.",
        status: "pass",
        severity: "low",
        fixPrompt: null,
        manualFix: null,
        rawData: null,
        confidence: "high",
      },
    ],
    detectedStack: null,
    durationMs: 1000,
    timestamp: "2026-05-13T10:00:00Z",
    ...overrides,
  };
}

function codeResult(overrides: Partial<CodeScanResult> = {}): CodeScanResult {
  return {
    id: 44,
    projectId: 7,
    environmentUrl: "https://example.com",
    overallScore: 81,
    issueCount: 3,
    criticalCount: 1,
    highCount: 1,
    mediumCount: 1,
    lowCount: 0,
    durationMs: 1200,
    checkedAt: "2026-05-13T10:01:00Z",
    framework: "Astro",
    domainSummaries: [],
    issues: [
      {
        id: "hardcoded-secret:src/env.ts",
        checkId: "code_scan.hardcoded-secret",
        category: "security",
        domain: "security",
        severity: "critical",
        title: "Hardcoded secret",
        description: "A secret appears in source.",
        relativePath: "src/env.ts",
        absolutePath: "/tmp/project/src/env.ts",
        line: 4,
        sourceExcerpt: null,
        evidence: null,
        whyNow: null,
        likelyFix: null,
        confidence: "high",
        verifyHint: null,
      },
      {
        id: "raw-sql:src/db.ts",
        checkId: "code_scan.raw-sql",
        category: "security",
        domain: "security",
        severity: "high",
        title: "Unsafe raw SQL",
        description: "SQL is built from raw input.",
        relativePath: "src/db.ts",
        absolutePath: "/tmp/project/src/db.ts",
        line: 18,
        sourceExcerpt: null,
        evidence: null,
        whyNow: null,
        likelyFix: null,
        confidence: "high",
        verifyHint: null,
      },
      {
        id: "ai-timeout:src/ai.ts",
        checkId: "code_scan.ai-timeout",
        category: "ai-safety",
        domain: "ai-safety",
        severity: "medium",
        title: "AI call has no timeout",
        description: "AI requests can hang indefinitely.",
        relativePath: "src/ai.ts",
        absolutePath: "/tmp/project/src/ai.ts",
        line: 42,
        sourceExcerpt: null,
        evidence: null,
        whyNow: null,
        likelyFix: null,
        confidence: "high",
        verifyHint: null,
      },
    ],
    ...overrides,
  };
}

describe("buildScanSummaryModel", () => {
  it("summarizes full scans with web and code issue deltas", () => {
    const history: ScanSummary[] = [
      {
        id: 11,
        url: "https://example.com",
        mode: "live",
        scanType: "health",
        overallScore: 80,
        issuesTotal: 1,
        issuesCritical: 0,
        issuesHigh: 1,
        issuesMedium: 0,
        issuesLow: 0,
        durationMs: 900,
        timestamp: "2026-05-12T10:00:00Z",
        sessionId: null,
        pageUrl: null,
      },
    ];
    const codeHistory: CodeScanSummary[] = [
      {
        id: 33,
        projectId: 7,
        environmentUrl: "https://example.com",
        overallScore: 77,
        issueCount: 5,
        groupedIssueCount: 5,
        criticalCount: 1,
        highCount: 2,
        durationMs: 1100,
        checkedAt: "2026-05-12T10:01:00Z",
        framework: "Astro",
        topDomain: "security",
        topDomainCount: 3,
        domainSummaries: [],
      },
    ];

    const summary = buildScanSummaryModel({
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 47,
      history,
      codeHistory,
      sessions: [],
      scopeLabel: "Example",
    });

    expect(summary?.title).toBe("Full scan complete");
    expect(summary?.scopeLabel).toBe("example.com");
    // There is exactly one score: the unified SiteCMD Score (the live snapshot).
    expect(summary?.siteCmdScore).toBe(47);
    expect(summary?.totalIssues).toBe(5);
    expect(summary?.severityCounts).toEqual({ critical: 1, high: 2, medium: 2, low: 0 });
    expect(summary?.estimatedNewIssues).toBe(1);
    expect(summary?.resolvedIssues).toBe(2);
    expect(summary?.regressionCount).toBe(1);
    // Negative control: the model exposes no per-source score decomposition.
    expect("sections" in (summary ?? {})).toBe(false);
    expect("scanScore" in (summary ?? {})).toBe(false);
    expect("projectScore" in (summary ?? {})).toBe(false);
  });

  it("uses the backend SiteCMD score as the single headline score when supplied", () => {
    const summary = buildScanSummaryModel({
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 25,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "Example",
    });

    expect(summary?.siteCmdScore).toBe(25);
  });

  it("has no SiteCMD score when the persisted score is unavailable", () => {
    const summary = buildScanSummaryModel({
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: null,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "Example",
    });

    // No project context supplied a score, so the single score is null.
    expect(summary?.siteCmdScore).toBeNull();
  });

  it("dedupes counts like the Issues list so the overview headline matches it", () => {
    const summary = buildScanSummaryModel({
      result: webResult({
        issues: [
          {
            checkId: "hsts",
            category: "security",
            title: "Missing HSTS",
            description: "No HSTS header.",
            status: "fail",
            severity: "high",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
            confidence: "high",
          },
          // Same check_id reported again (e.g. on another page) collapses to one.
          {
            checkId: "hsts",
            category: "security",
            title: "Missing HSTS",
            description: "No HSTS header.",
            status: "fail",
            severity: "high",
            fixPrompt: null,
            manualFix: null,
            rawData: null,
            confidence: "high",
          },
        ],
      }),
      codeResult: codeResult({
        // Backend raw aggregate says 4, but the two occurrences share one
        // canonical check id and group to one, like the list.
        issueCount: 4,
        criticalCount: 0,
        highCount: 0,
        mediumCount: 4,
        lowCount: 0,
        issues: [
          {
            id: "missing-input-validation:src/a.ts",
            checkId: "code_scan.missing-input-validation",
            category: "security",
            domain: "security",
            severity: "medium",
            title: "Missing input validation",
            description: "Input is not validated.",
            relativePath: "src/a.ts",
            absolutePath: "/tmp/project/src/a.ts",
            line: 4,
            sourceExcerpt: null,
            evidence: null,
            whyNow: null,
            likelyFix: null,
            confidence: "high",
            verifyHint: null,
          },
          {
            id: "missing-input-validation:src/b.ts",
            checkId: "code_scan.missing-input-validation",
            category: "security",
            domain: "security",
            severity: "medium",
            title: "Missing input validation",
            description: "Input is not validated.",
            relativePath: "src/b.ts",
            absolutePath: "/tmp/project/src/b.ts",
            line: 9,
            sourceExcerpt: null,
            evidence: null,
            whyNow: null,
            likelyFix: null,
            confidence: "high",
            verifyHint: null,
          },
        ],
      }),
      multiResult: null,
      sitecmdScore: 50,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "Example",
    });

    // Raw would be 2 web + 4 code = 6; deduped is 1 web + 1 code = 2.
    expect(summary?.totalIssues).toBe(2);
    expect(summary?.severityCounts).toEqual({ critical: 0, high: 1, medium: 1, low: 0 });
  });

  it("excludes inactive check_ids so the overview matches the active list", () => {
    const summary = buildScanSummaryModel({
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 60,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "Example",
      // Block the high web issue (by check_id) and the critical code issue (by
      // checkId) -- the same exclusion the score and the active list apply.
      inactiveCheckIds: new Set(["hsts", "code_scan.hardcoded-secret"]),
    });

    // Without the exclusion this is 5 issues with 1 critical; both blocked
    // issues drop from the headline total and severity counts.
    expect(summary?.totalIssues).toBe(3);
    expect(summary?.severityCounts).toEqual({ critical: 0, high: 1, medium: 2, low: 0 });
  });

  it("uses the persisted summary for the headline so the overview matches the sidebar and list", () => {
    const summary = buildScanSummaryModel({
      // The raw scan result would total 2 web + 3 code = 5 issues...
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 50,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "Example",
      persistedSummary: {
        webCount: 2,
        codeCount: 1,
        totalCount: 3,
        criticalCount: 1,
        severityCounts: { critical: 1, high: 1, medium: 1, low: 0 },
      },
    });

    expect(summary?.totalIssues).toBe(3);
    expect(summary?.severityCounts).toEqual({ critical: 1, high: 1, medium: 1, low: 0 });
  });

  it("keeps the overview severity chips summing to the headline when the code fallback carries raw crit/high", () => {
    // Active grouped totals must cap stale raw severity counts.
    const persistedSummary = buildProjectIssueSummary({
      webIssues: Array.from({ length: 9 }, () => ({ severity: "low" })),
      codeIssues: [],
      codeSummaryFallback: {
        issueCount: 2,
        criticalCount: 2,
        highCount: 26,
        mode: "summary",
      },
    });

    const summary = buildScanSummaryModel({
      result: webResult(),
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 40,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "SiteCMD marketing",
      persistedSummary,
    });

    const severityTotal =
      (summary?.severityCounts.critical ?? 0) +
      (summary?.severityCounts.high ?? 0) +
      (summary?.severityCounts.medium ?? 0) +
      (summary?.severityCounts.low ?? 0);

    expect(summary?.totalIssues).toBe(11);
    expect(severityTotal).toBe(summary?.totalIssues);
    expect(summary?.severityCounts).toEqual({ critical: 2, high: 0, medium: 0, low: 9 });
  });

  it("summarizes multi-page scans without a previous session", () => {
    const multiResult: MultiScanResult = {
      sessionId: 9,
      totalPages: 3,
      completedPages: 3,
      overallScore: 88,
      durationMs: 2300,
      newIssueCount: null,
      resolvedIssueCount: null,
      siteIssues: [],
      pageResults: [
        {
          url: "https://example.com",
          score: 90,
          issuesCount: 1,
          issuesCritical: 0,
          issuesHigh: 1,
          issuesMedium: 0,
          issuesLow: 0,
          durationMs: 700,
          scanId: 1,
        },
        {
          url: "https://example.com/about",
          score: 86,
          issuesCount: 2,
          issuesCritical: 0,
          issuesHigh: 0,
          issuesMedium: 1,
          issuesLow: 1,
          durationMs: 800,
          scanId: 2,
        },
      ],
    };
    const sessions: ScanSessionSummary[] = [];

    const summary = buildScanSummaryModel({
      result: null,
      codeResult: null,
      multiResult,
      sitecmdScore: null,
      history: [],
      codeHistory: [],
      sessions,
      scopeLabel: "Example",
    });

    expect(summary?.title).toBe("Page scan complete");
    expect(summary?.totalIssues).toBe(3);
    expect(summary?.estimatedNewIssues).toBeNull();
    // No project score supplied for this page scan, so the single score is null.
    expect(summary?.siteCmdScore).toBeNull();
  });

  it("keeps the persisted active issue total authoritative for multi-page scans", () => {
    const summary = buildScanSummaryModel({
      result: null,
      codeResult: null,
      multiResult: {
        sessionId: 10,
        totalPages: 2,
        completedPages: 2,
        overallScore: 84,
        durationMs: 1_500,
        newIssueCount: 2,
        resolvedIssueCount: 1,
        siteIssues: [],
        pageResults: [
          {
            url: "https://example.com",
            score: 86,
            issuesCount: 5,
            issuesCritical: 0,
            issuesHigh: 1,
            issuesMedium: 3,
            issuesLow: 1,
            durationMs: 700,
            scanId: 11,
          },
          {
            url: "https://example.com/about",
            score: 82,
            issuesCount: 4,
            issuesCritical: 0,
            issuesHigh: 0,
            issuesMedium: 2,
            issuesLow: 2,
            durationMs: 800,
            scanId: 12,
          },
        ],
      },
      sitecmdScore: 76,
      history: [],
      codeHistory: [],
      sessions: [
        {
          sessionId: 9,
          totalPages: 1,
          completedPages: 1,
          status: "complete",
          startedAt: "2026-05-12T10:00:00Z",
          overallScore: 90,
          durationMs: 600,
          pageScans: [
            {
              id: 10,
              url: "https://example.com",
              mode: "live",
              scanType: "health",
              overallScore: 90,
              issuesTotal: 0,
              issuesCritical: 0,
              issuesHigh: 0,
              issuesMedium: 0,
              issuesLow: 0,
              durationMs: 600,
              timestamp: "2026-05-12T10:00:00Z",
              sessionId: 9,
              pageUrl: "https://example.com",
            },
          ],
        },
      ],
      scopeLabel: "Example",
      persistedSummary: {
        webCount: 2,
        codeCount: 1,
        totalCount: 3,
        criticalCount: 0,
        severityCounts: { critical: 0, high: 1, medium: 2, low: 0 },
      },
    });

    expect(summary?.totalIssues).toBe(3);
    expect(summary?.severityCounts).toEqual({ critical: 0, high: 1, medium: 2, low: 0 });
    expect(summary?.estimatedNewIssues).toBe(2);
    expect(summary?.resolvedIssues).toBe(1);
  });
});

describe("buildSkippedScopeNote (D6 carryover)", () => {
  it("returns null when nothing was skipped or the tally is absent (history reload)", () => {
    expect(buildSkippedScopeNote(undefined)).toBeNull();
    expect(
      buildSkippedScopeNote({
        nestedRepositories: 0,
        gitignoredDirectories: 0,
        sampleNames: [],
      }),
    ).toBeNull();
  });

  it("names nested repositories and gitignored trees with a sample", () => {
    const note = buildSkippedScopeNote({
      nestedRepositories: 2,
      gitignoredDirectories: 1,
      sampleNames: ["vendor-clone", "packages/api", "build"],
    });
    expect(note).toMatch(/2 nested repositories/);
    expect(note).toMatch(/1 gitignored directory/);
    expect(note).toMatch(/vendor-clone, packages\/api, build/);
    expect(note).toMatch(/not scanned as this project's code/i);
  });

  it("singularizes a single nested repository", () => {
    const note = buildSkippedScopeNote({
      nestedRepositories: 1,
      gitignoredDirectories: 0,
      sampleNames: ["submodule"],
    });
    expect(note).toMatch(/1 nested repository\b/);
    // The count clause is singular (the static explainer sentence still says
    // "Nested repositories..."), so only guard the count phrasing.
    expect(note).not.toMatch(/1 nested repositories/);
  });
});

describe("buildScanSummaryModel skipped-scope note", () => {
  it("hoists the skipped-scope note to the top level of the model", () => {
    const summary = buildScanSummaryModel({
      result: null,
      codeResult: codeResult({
        skippedScopes: {
          nestedRepositories: 3,
          gitignoredDirectories: 0,
          sampleNames: ["repo-a", "repo-b", "repo-c"],
        },
      }),
      multiResult: null,
      sitecmdScore: 70,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "example.com",
    });
    expect(summary?.note).toMatch(/3 nested repositories/);
  });

  it("leaves the note null when the code scan skipped nothing", () => {
    const summary = buildScanSummaryModel({
      result: null,
      codeResult: codeResult(),
      multiResult: null,
      sitecmdScore: 70,
      history: [],
      codeHistory: [],
      sessions: [],
      scopeLabel: "example.com",
    });
    expect(summary?.note).toBeNull();
  });
});
