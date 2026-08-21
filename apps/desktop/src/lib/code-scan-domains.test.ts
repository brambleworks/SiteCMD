import { describe, expect, it } from "vitest";
import {
  CODE_SCAN_DOMAIN_META,
  CODE_SCAN_DOMAIN_ORDER,
  getCodeIssueDomain,
  type ClassifiableCodeIssue,
} from "./code-scan-domains";
import { getCodeScanDomainFocus, getCodeScanDomainFromFocus } from "./app-targets";

function issue(overrides: Partial<ClassifiableCodeIssue> = {}): ClassifiableCodeIssue {
  return {
    id: "test-issue",
    checkId: "code_scan.test-issue",
    category: "architecture",
    severity: "medium",
    title: "Test",
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

describe("CODE_SCAN_DOMAIN_META", () => {
  it("has a meta entry for every domain in CODE_SCAN_DOMAIN_ORDER", () => {
    for (const domain of CODE_SCAN_DOMAIN_ORDER) {
      const meta = CODE_SCAN_DOMAIN_META[domain];
      expect(meta).toBeDefined();
      expect(meta.label.length).toBeGreaterThan(0);
      expect(meta.shortLabel.length).toBeGreaterThan(0);
      expect(meta.description.length).toBeGreaterThan(0);
      expect(["function", "object"]).toContain(typeof meta.icon);
      expect(meta.accentVar.length).toBeGreaterThan(0);
    }
  });

  it("exposes security with the --cat-security accent (shared with web scans)", () => {
    expect(CODE_SCAN_DOMAIN_META.security.accentVar).toBe("--cat-security");
  });

  it("uses the clearer Dependencies label for supply-chain issues", () => {
    expect(CODE_SCAN_DOMAIN_META["supply-chain"].label).toBe("Dependencies");
    expect(CODE_SCAN_DOMAIN_META["supply-chain"].shortLabel).toBe("Dependencies");
  });
});

describe("getCodeIssueDomain - explicit domain field wins", () => {
  it("respects the issue.domain field regardless of category/id", () => {
    const x = issue({
      id: "literally-anything",
      category: "architecture",
      domain: "database",
    });
    expect(getCodeIssueDomain(x)).toBe("database");
  });
});

describe("getCodeIssueDomain - ai-safety detection", () => {
  it("classifies category=ai-safety as ai-safety", () => {
    expect(getCodeIssueDomain(issue({ category: "ai-safety" }))).toBe("ai-safety");
  });

  it("classifies issues with id starting 'ai-' as ai-safety even when category is wrong", () => {
    expect(getCodeIssueDomain(issue({ id: "ai-timeout", category: "architecture" }))).toBe(
      "ai-safety",
    );
  });
});

describe("getCodeIssueDomain - database detection", () => {
  it("classifies category=data as database", () => {
    expect(getCodeIssueDomain(issue({ category: "data" }))).toBe("database");
  });

  it("classifies issues with a known DATABASE_ID_PREFIX id as database", () => {
    const prefixes = [
      "local-sqlite-",
      "local-postgres-",
      "local-prisma-",
      "supabase-",
      "schema-join-",
      "unsafe-raw-sql",
      "interpolated-sql",
      "formatted-sql",
    ];
    for (const prefix of prefixes) {
      const result = getCodeIssueDomain(
        issue({ id: `${prefix}my-thing`, category: "architecture" }),
      );
      expect(result).toBe("database");
    }
  });

  it("classifies issues containing database hint words as database", () => {
    expect(
      getCodeIssueDomain(
        issue({
          id: "something-else",
          title: "Migration step missing foreign key constraint",
          category: "architecture",
        }),
      ),
    ).toBe("database");

    expect(
      getCodeIssueDomain(
        issue({
          id: "another-check",
          description: "This query does not scope by tenant",
          category: "architecture",
        }),
      ),
    ).toBe("database");
  });
});

describe("getCodeIssueDomain - category fallthrough", () => {
  it("classifies category=security as security (when no database/ai signal)", () => {
    expect(getCodeIssueDomain(issue({ category: "security" }))).toBe("security");
  });

  it("classifies category=supply-chain as supply-chain", () => {
    expect(getCodeIssueDomain(issue({ category: "supply-chain" }))).toBe("supply-chain");
  });

  it("classifies category=operations as operations", () => {
    expect(getCodeIssueDomain(issue({ category: "operations" }))).toBe("operations");
  });

  it("falls through to architecture when nothing else matches", () => {
    expect(
      getCodeIssueDomain(
        issue({
          id: "god-route",
          category: "architecture",
          title: "Route handler has too many responsibilities",
          description: "Split into smaller modules",
        }),
      ),
    ).toBe("architecture");
  });
});

describe("getCodeIssueDomain - precedence ordering", () => {
  it("ai-safety wins over database when both match", () => {
    expect(
      getCodeIssueDomain(
        issue({
          id: "ai-sql-hallucination",
          title: "AI generated an interpolated-sql query",
          category: "architecture",
        }),
      ),
    ).toBe("ai-safety");
  });

  it("database wins over plain category=security when database hints are present", () => {
    expect(
      getCodeIssueDomain(
        issue({
          id: "some-check",
          title: "Missing row level security policy",
          category: "security",
        }),
      ),
    ).toBe("database");
  });
});

function codeIssue(overrides: Partial<ClassifiableCodeIssue>): ClassifiableCodeIssue {
  return {
    id: "agent-instructions-stub:CLAUDE.md",
    checkId: "code_scan.agent-instructions-stub",
    category: "ai-scaffolding",
    domain: null,
    severity: "low",
    title: "CLAUDE.md has almost no guidance",
    description: "",
    relativePath: "CLAUDE.md",
    absolutePath: "/tmp/CLAUDE.md",
    line: 1,
    sourceExcerpt: null,
    evidence: null,
    whyNow: null,
    likelyFix: null,
    confidence: "high",
    verifyHint: null,
    ...overrides,
  };
}

describe("ai-scaffolding domain", () => {
  it("is a known, ordered domain with metadata", () => {
    expect(CODE_SCAN_DOMAIN_ORDER).toContain("ai-scaffolding");
    expect(CODE_SCAN_DOMAIN_META["ai-scaffolding"].label).toBeTruthy();
  });

  it("routes ai-scaffolding issues by category when no domain is stamped", () => {
    expect(getCodeIssueDomain(codeIssue({ domain: null }))).toBe("ai-scaffolding");
  });

  it("round-trips through the Issues deep-link focus", () => {
    const focus = getCodeScanDomainFocus("ai-scaffolding");
    expect(focus).toBe("code-scan-domain:ai-scaffolding");
    expect(getCodeScanDomainFromFocus(focus)).toBe("ai-scaffolding");
  });
});
