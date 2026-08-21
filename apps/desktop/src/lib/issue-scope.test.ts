import { describe, expect, it } from "vitest";
import { getCheckIssueScope, getGuardrailIssueScope, getLaunchItemScope } from "./issue-scope";
import type { CheckResult, CodeIssue } from "./types";

function check(overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    checkId: "security.csp",
    category: "security",
    title: "Missing CSP",
    description: "",
    status: "fail",
    severity: "high",
    fixPrompt: null,
    manualFix: null,
    rawData: null,
    confidence: "high",
    ...overrides,
  };
}

function codeIssue(overrides: Partial<CodeIssue> = {}): CodeIssue {
  return {
    id: "test",
    checkId: "code_scan.test",
    category: "ai-safety",
    domain: "ai-safety",
    severity: "high",
    title: "Issue",
    description: "",
    relativePath: "src/foo.ts",
    absolutePath: "/tmp/src/foo.ts",
    line: null,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
    ...overrides,
  };
}

describe("getCheckIssueScope", () => {
  it("classifies CSP / HSTS / cookie checks as sitewide", () => {
    expect(getCheckIssueScope(check({ checkId: "security.csp" })).scope).toBe("site");
    expect(getCheckIssueScope(check({ checkId: "security.hsts" })).scope).toBe("site");
    expect(getCheckIssueScope(check({ checkId: "security.cookie_flags" })).scope).toBe("site");
  });

  it("classifies accessibility / alt / heading checks as page-scoped", () => {
    expect(
      getCheckIssueScope(check({ checkId: "accessibility.alt_text", category: "accessibility" }))
        .scope,
    ).toBe("page");
    expect(getCheckIssueScope(check({ checkId: "seo.heading_order", category: "seo" })).scope).toBe(
      "page",
    );
    expect(
      getCheckIssueScope(check({ checkId: "perf.dom_size", category: "performance" })).scope,
    ).toBe("page");
  });

  it("falls back to category when the check_id doesn't match any pattern", () => {
    expect(getCheckIssueScope(check({ checkId: "misc.unknown", category: "security" })).scope).toBe(
      "site",
    );
    expect(
      getCheckIssueScope(check({ checkId: "misc.unknown", category: "accessibility" })).scope,
    ).toBe("page");
  });

  it("derives page subject from raw_data.page_url when present", () => {
    const meta = getCheckIssueScope(
      check({
        checkId: "accessibility.alt",
        category: "accessibility",
        rawData: { page_url: "https://example.com/about/team?tab=1" },
      }),
    );
    expect(meta.scope).toBe("page");
    expect(meta.subjectLabel).toBe("/about/team?tab=1");
  });

  it("derives site subject from the scan URL as the host", () => {
    const meta = getCheckIssueScope(
      check({ checkId: "security.csp" }),
      "https://www.example.com/foo",
    );
    expect(meta.scope).toBe("site");
    expect(meta.subjectLabel).toBe("www.example.com");
  });

  it("returns '/' when a page_url has no path", () => {
    const meta = getCheckIssueScope(
      check({
        checkId: "accessibility.alt",
        category: "accessibility",
        rawData: { page_url: "https://example.com" },
      }),
    );
    expect(meta.subjectLabel).toBe("/");
  });

  it("sets the issue label to Page / Sitewide / Code in lockstep with scope", () => {
    const page = getCheckIssueScope(
      check({ checkId: "accessibility.alt", category: "accessibility" }),
    );
    const site = getCheckIssueScope(check({ checkId: "security.csp" }));
    expect(page.issueLabel).toBe("Page issue");
    expect(site.issueLabel).toBe("Sitewide issue");
    expect(page.scopeLabel).toBe("Page");
    expect(site.scopeLabel).toBe("Sitewide");
  });
});

describe("getGuardrailIssueScope", () => {
  it("always tags code-scan issues as code-scoped", () => {
    const meta = getGuardrailIssueScope(codeIssue());
    expect(meta.scope).toBe("code");
    expect(meta.scopeLabel).toBe("Code");
    expect(meta.issueLabel).toBe("Code issue");
  });

  it("renders 'relativePath:line' when a line number is present", () => {
    const meta = getGuardrailIssueScope(codeIssue({ relativePath: "app/api/route.ts", line: 42 }));
    expect(meta.subjectLabel).toBe("app/api/route.ts:42");
  });

  it("falls back to just the path when line is null", () => {
    const meta = getGuardrailIssueScope(codeIssue({ relativePath: "package.json", line: null }));
    expect(meta.subjectLabel).toBe("package.json");
  });
});

describe("getLaunchItemScope", () => {
  it("classifies items mentioning source files as code-scoped", () => {
    const meta = getLaunchItemScope({
      id: "launch.fix-1",
      label: "Fix this",
      description: "Check src/routes/auth.ts for missing auth",
      fixHint: "Edit src/routes/auth.ts and add middleware",
    });
    expect(meta.scope).toBe("code");
    // subjectLabel is extracted from details/fixHint/fixPrompt, not description
    expect(meta.subjectLabel).toBe("src/routes/auth.ts");
  });

  it("classifies accessibility/perf-prefixed ids as page-scoped", () => {
    const meta = getLaunchItemScope({
      id: "accessibility.missing-alt",
      label: "Alt text",
      description: "",
    });
    expect(meta.scope).toBe("page");
  });

  it("classifies sec/infra/analytics-prefixed ids as sitewide", () => {
    expect(getLaunchItemScope({ id: "sec.csp", label: "CSP", description: "" }).scope).toBe("site");
    expect(getLaunchItemScope({ id: "infra.dns", label: "DNS", description: "" }).scope).toBe(
      "site",
    );
    expect(getLaunchItemScope({ id: "analytics.ga4", label: "GA4", description: "" }).scope).toBe(
      "site",
    );
  });

  it("extracts the code path from fixHint when scope is code", () => {
    const meta = getLaunchItemScope({
      id: "launch.code-fix",
      label: "Unsafe query",
      description: "",
      fixHint: "See app/api/users/route.ts for the unsanitized query",
    });
    expect(meta.scope).toBe("code");
    expect(meta.subjectLabel).toBe("app/api/users/route.ts");
  });
});
