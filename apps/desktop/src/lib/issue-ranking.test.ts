import { describe, expect, it } from "vitest";
import {
  countGroupedCodeIssues,
  countGroupedWebIssues,
  findUnifiedByCheckId,
  rankUnified,
} from "@/lib/issue-ranking";
import { getCodeIssueDomain, type ClassifiableCodeIssue } from "@/lib/code-scan-domains";
import type { CheckResult, CodeIssue } from "@/lib/types";

function webIssue(overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    checkId: "security.csp",
    category: "security",
    title: "Missing CSP",
    description: "",
    status: "fail",
    severity: "critical",
    fixPrompt: null,
    manualFix: null,
    rawData: null,
    confidence: "high",
    ...overrides,
  };
}

function codeIssue(overrides: Partial<CodeIssue> = {}): CodeIssue {
  const base: ClassifiableCodeIssue = {
    id: "local-postgres-owner-scope",
    checkId: "code_scan.local-postgres-owner-scope",
    category: "data",
    severity: "critical",
    title: "Missing owner scope",
    description: "",
    relativePath: "app/api/route.ts",
    absolutePath: "/tmp/test/app/api/route.ts",
    line: null,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
    ...overrides,
  };
  return { ...base, domain: getCodeIssueDomain(base) };
}

describe("rankUnified", () => {
  it("sorts merged issues by impact, descending", () => {
    const web = [
      webIssue({
        checkId: "security.csp",
        category: "security",
        severity: "critical",
      }),
      webIssue({
        checkId: "performance.ttfb",
        category: "performance",
        severity: "high",
      }),
      webIssue({
        checkId: "seo.meta",
        category: "seo",
        severity: "medium",
      }),
    ];
    const code = [
      codeIssue({
        id: "db-scope",
        checkId: "code_scan.db-scope",
        title: "Missing owner scope",
        severity: "critical",
      }),
      codeIssue({
        id: "ops-timeout",
        checkId: "code_scan.ops-timeout",
        title: "Retry queue has no timeout",
        category: "operations",
        severity: "high",
      }),
    ];

    const ranked = rankUnified(web, code, [], {});

    expect(ranked.map((r) => r.id)).toEqual([
      "web:security.csp",
      "code-group:code_scan.db-scope",
      "web:performance.ttfb",
      "code-group:code_scan.ops-timeout",
      "web:seo.meta",
    ]);
  });

  it("computes web impact from the shared source-independent SiteCMD score model", () => {
    const ranked = rankUnified(
      [webIssue({ category: "security", severity: "critical" })],
      [],
      [],
      {},
    );

    // Default confidence is high, so critical impact is 25 × 0.85 → 21.
    expect(ranked[0].kind).toBe("web");
    expect(ranked[0].impact).toBe(21);
  });

  it("trusts scanner-provided severities for ranking and display", () => {
    const ranked = rankUnified(
      [
        webIssue({
          checkId: "polish.ai-buzzword-dictionary",
          category: "polish",
          severity: "low",
          title: "High Marketing Buzzword Density",
        }),
      ],
      [],
      [],
      {},
    );

    expect(ranked[0]).toMatchObject({
      kind: "web",
      issue: { severity: "low" },
      impact: 1,
    });
  });

  it("applies the same source-independent impact values to code issues", () => {
    const ranked = rankUnified(
      [],
      [
        codeIssue({ id: "c1", checkId: "code_scan.c1", severity: "critical" }),
        codeIssue({ id: "c2", checkId: "code_scan.c2", severity: "high" }),
        codeIssue({ id: "c3", checkId: "code_scan.c3", severity: "medium" }),
        codeIssue({ id: "c4", checkId: "code_scan.c4", severity: "low" }),
      ],
      [],
      {},
    );

    expect(ranked.map((r) => r.impact)).toEqual([21, 10, 4, 1]);
  });

  it("clamps tiny web impacts to a minimum of 1", () => {
    const ranked = rankUnified([webIssue({ category: "polish", severity: "low" })], [], [], {});

    // penalty 1.5 × weight 0.10 = 0.15 → round → 0 → clamped → 1
    expect(ranked[0].impact).toBe(1);
  });

  it("emits stable keys so React rows don't collide across sources", () => {
    const ranked = rankUnified(
      [webIssue({ checkId: "collision" })],
      [codeIssue({ id: "collision" })],
      [],
      {},
    );
    const ids = ranked.map((r) => r.id);
    expect(new Set(ids).size).toBe(2);
    expect(ids).toContain("web:collision");
    expect(ids).toContain("code-group:code_scan.local-postgres-owner-scope");
  });

  it("returns an empty array when both lists are empty", () => {
    expect(rankUnified([], [], [], {})).toEqual([]);
  });

  it("labels SEO issues correctly (all-caps, not 'Seo')", () => {
    const ranked = rankUnified([webIssue({ category: "seo", severity: "high" })], [], [], {});
    expect(ranked[0].sourceLabel).toBe("SEO");
  });

  it("groups duplicate web issues by check id", () => {
    const ranked = rankUnified(
      [
        webIssue({ checkId: "security.hsts", rawData: { path: "/settings" } }),
        webIssue({ checkId: "security.hsts", rawData: { path: "/billing" } }),
      ],
      [],
      [],
      {},
    );

    expect(ranked).toHaveLength(1);
    expect(ranked[0]).toMatchObject({
      kind: "web",
      id: "web:security.hsts",
      occurrenceCount: 2,
      occurrenceLabels: ["/billing", "/settings"],
    });
  });

  it("groups Code occurrences by canonical check id", () => {
    const ranked = rankUnified(
      [],
      [
        codeIssue({
          id: "rate-limit-a",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          severity: "high",
          title: "Public-facing route has no clear rate limiting",
          relativePath: "app/api/foo/route.ts",
          line: 12,
        }),
        codeIssue({
          id: "rate-limit-b",
          checkId: "code_scan.public-endpoint-rate-limit",
          category: "security",
          severity: "high",
          title: "Public-facing route has no clear rate limiting",
          relativePath: "app/api/bar/route.ts",
          line: 44,
        }),
      ],
      [],
      {},
    );

    expect(ranked).toHaveLength(1);
    expect(ranked[0]).toMatchObject({
      kind: "code",
      id: "code-group:code_scan.public-endpoint-rate-limit",
      occurrenceCount: 2,
      occurrenceLabels: ["app/api/bar/route.ts:44", "app/api/foo/route.ts:12"],
    });
  });

  it("keeps Code row identity stable across title and severity changes", () => {
    const ranked = rankUnified(
      [],
      [
        codeIssue({
          id: "rule-a:src/a.ts",
          checkId: "code_scan.rule-a",
          title: "Old wording",
          severity: "medium",
        }),
        codeIssue({
          id: "rule-a:src/b.ts",
          checkId: "code_scan.rule-a",
          title: "Renamed wording",
          severity: "high",
          relativePath: "src/b.ts",
        }),
      ],
      [],
      {},
    );

    expect(ranked).toHaveLength(1);
    expect(ranked[0]).toMatchObject({
      id: "code-group:code_scan.rule-a",
      occurrenceCount: 2,
      issue: { severity: "high", title: "Renamed wording" },
    });
  });

  it("navigates to a Code row by canonical check id", () => {
    const ranked = rankUnified(
      [],
      [codeIssue({ checkId: "code_scan.rule-a", title: "Any title" })],
      [],
      {},
    );

    expect(findUnifiedByCheckId(ranked, "code_scan.rule-a")?.id).toBe(
      "code-group:code_scan.rule-a",
    );
  });

  it("shares grouped issue counts with count helpers", () => {
    expect(countGroupedWebIssues([{ checkId: "a" }, { checkId: "a" }, { checkId: "b" }])).toBe(2);

    expect(
      countGroupedCodeIssues([
        {
          checkId: "code_scan.public-endpoint-rate-limit",
          severity: "high",
        },
        {
          checkId: "code_scan.public-endpoint-rate-limit",
          severity: "high",
        },
        {
          checkId: "code_scan.supply-chain-typosquat",
          severity: "critical",
        },
      ]),
    ).toBe(2);
  });
});
