import { describe, expect, it } from "vitest";
import {
  CODE_SCAN_DOMAIN_FOCUS_PREFIX,
  CODE_SCAN_FOCUS,
  getCodeScanDomainFocus,
  getCodeScanDomainFromFocus,
  getIssuesStatusFocus,
  getIssuesStatusFromFocus,
  getIssuesWebCategoryFocus,
  getIssuesWebCategoryFromFocus,
  isCodeScanFocus,
  normalizeAppUrlForKey,
  normalizeAppUrlForOptionalKey,
  normalizeHttpTargetUrl,
  normalizeTargetUrl,
  withNormalizedTarget,
} from "./app-targets";

describe("isCodeScanFocus", () => {
  it("matches the bare CODE_SCAN_FOCUS literal", () => {
    expect(isCodeScanFocus(CODE_SCAN_FOCUS)).toBe(true);
  });

  it("matches any domain-prefixed focus", () => {
    expect(isCodeScanFocus("code-scan-domain:database")).toBe(true);
    expect(isCodeScanFocus("code-scan-domain:ai-safety")).toBe(true);
  });

  it("rejects unrelated focuses and null/undefined", () => {
    expect(isCodeScanFocus(null)).toBe(false);
    expect(isCodeScanFocus(undefined)).toBe(false);
    expect(isCodeScanFocus("")).toBe(false);
    expect(isCodeScanFocus("security.csp")).toBe(false);
  });
});

describe("getCodeScanDomainFocus / getCodeScanDomainFromFocus", () => {
  it("round-trips every valid domain", () => {
    const domains = [
      "database",
      "ai-safety",
      "security",
      "architecture",
      "operations",
      "supply-chain",
    ] as const;
    for (const domain of domains) {
      const focus = getCodeScanDomainFocus(domain);
      expect(focus.startsWith(CODE_SCAN_DOMAIN_FOCUS_PREFIX)).toBe(true);
      expect(getCodeScanDomainFromFocus(focus)).toBe(domain);
    }
  });

  it("returns null when the focus prefix is wrong", () => {
    expect(getCodeScanDomainFromFocus("security.csp")).toBeNull();
    expect(getCodeScanDomainFromFocus(null)).toBeNull();
    expect(getCodeScanDomainFromFocus(undefined)).toBeNull();
    expect(getCodeScanDomainFromFocus("")).toBeNull();
  });

  it("returns null when the domain segment is not in the known set", () => {
    expect(getCodeScanDomainFromFocus(`${CODE_SCAN_DOMAIN_FOCUS_PREFIX}mystery-domain`)).toBeNull();
  });
});

describe("getIssuesStatusFocus / getIssuesStatusFromFocus", () => {
  it("round-trips issue statuses", () => {
    expect(getIssuesStatusFromFocus(getIssuesStatusFocus("blocked"))).toBe("blocked");
    expect(getIssuesStatusFromFocus(getIssuesStatusFocus("active"))).toBe("active");
  });

  it("returns null for unrelated or invalid status focuses", () => {
    expect(getIssuesStatusFromFocus("issues-status:paused")).toBeNull();
    expect(getIssuesStatusFromFocus("issues-source:web")).toBeNull();
    expect(getIssuesStatusFromFocus(null)).toBeNull();
  });
});

describe("getIssuesWebCategoryFocus / getIssuesWebCategoryFromFocus", () => {
  it("round-trips every valid web category", () => {
    const categories = [
      "security",
      "performance",
      "seo",
      "accessibility",
      "compliance",
      "config",
      "polish",
    ] as const;
    for (const category of categories) {
      const focus = getIssuesWebCategoryFocus(category);
      expect(getIssuesWebCategoryFromFocus(focus)).toBe(category);
    }
  });

  it("returns null for unrelated or invalid focuses", () => {
    expect(getIssuesWebCategoryFromFocus("security")).toBeNull();
    expect(getIssuesWebCategoryFromFocus("issues-web-category:mystery")).toBeNull();
    expect(getIssuesWebCategoryFromFocus(null)).toBeNull();
    expect(getIssuesWebCategoryFromFocus(undefined)).toBeNull();
  });
});

describe("normalizeTargetUrl", () => {
  it("strips a trailing slash", () => {
    expect(normalizeTargetUrl("https://example.com/")).toBe("https://example.com");
  });

  it("leaves a URL without trailing slash untouched", () => {
    expect(normalizeTargetUrl("https://example.com")).toBe("https://example.com");
  });

  it("returns null for null / undefined / empty", () => {
    expect(normalizeTargetUrl(null)).toBeNull();
    expect(normalizeTargetUrl(undefined)).toBeNull();
    expect(normalizeTargetUrl("")).toBeNull();
  });

  it("only strips one trailing slash, leaves path intact", () => {
    expect(normalizeTargetUrl("https://example.com/about/")).toBe("https://example.com/about");
    expect(normalizeTargetUrl("https://example.com/about")).toBe("https://example.com/about");
  });

  it("rejects unsafe target URL values", () => {
    expect(normalizeTargetUrl("javascript:alert(1)")).toBeNull();
    expect(normalizeTargetUrl("https://user:token@example.com")).toBeNull();
  });
});

describe("normalizeHttpTargetUrl", () => {
  it("normalizes safe http and https target URLs", () => {
    expect(normalizeHttpTargetUrl(" https://example.com/about/ ")).toBe(
      "https://example.com/about",
    );
    expect(normalizeHttpTargetUrl("http://localhost:4321/")).toBe("http://localhost:4321");
  });

  it("rejects non-http and credential-bearing target URLs", () => {
    expect(normalizeHttpTargetUrl("javascript:alert(1)")).toBeNull();
    expect(normalizeHttpTargetUrl("file:///Users/dev/private.txt")).toBeNull();
    expect(normalizeHttpTargetUrl("https://user:token@example.com")).toBeNull();
  });
});

describe("normalizeAppUrlForKey", () => {
  it("normalizes http URLs consistently for cache and work-item keys", () => {
    expect(normalizeAppUrlForKey(" https://Example.com/about/ ")).toBe("https://example.com/about");
    expect(normalizeAppUrlForKey("http://localhost:4321/")).toBe("http://localhost:4321");
  });

  it("keeps the legacy non-http fallback in one place", () => {
    expect(normalizeAppUrlForKey("sitecmd-preview/")).toBe("sitecmd-preview");
  });

  it("has explicit empty and optional forms", () => {
    expect(normalizeAppUrlForKey(null)).toBe("");
    expect(normalizeAppUrlForOptionalKey(null)).toBeNull();
    expect(normalizeAppUrlForOptionalKey("https://example.com/")).toBe("https://example.com");
  });

  it("matches the Rust normalize_url contract: lowercase host, preserve path case", () => {
    expect(normalizeAppUrlForKey("https://Example.COM/About/")).toBe("https://example.com/About");
    expect(normalizeAppUrlForKey("Https://SiteCMD.com")).toBe("https://sitecmd.com");
  });
});

describe("withNormalizedTarget", () => {
  it("fills all optional fields to null and normalizes the URL", () => {
    const normalized = withNormalizedTarget({
      page: "issues",
      url: "https://example.com/",
    });
    expect(normalized.url).toBe("https://example.com");
    expect(normalized.scanId).toBeNull();
    expect(normalized.sessionId).toBeNull();
    expect(normalized.scanKind).toBeNull();
    expect(normalized.focus).toBeNull();
    expect(normalized.itemId).toBeNull();
    expect(normalized.promptId).toBeNull();
    expect(normalized.lane).toBeNull();
    expect(normalized.reason).toBeNull();
    expect(normalized.filePath).toBeNull();
  });

  it("preserves fields that are already set", () => {
    const normalized = withNormalizedTarget({
      page: "issues",
      projectId: 42,
      scanId: 5,
      sessionId: 9,
      scanKind: "code",
      focus: CODE_SCAN_FOCUS,
      itemId: "issue-123",
      reason: "fresh scan",
    });
    expect(normalized.projectId).toBe(42);
    expect(normalized.scanId).toBe(5);
    expect(normalized.sessionId).toBe(9);
    expect(normalized.scanKind).toBe("code");
    expect(normalized.focus).toBe(CODE_SCAN_FOCUS);
    expect(normalized.itemId).toBe("issue-123");
    expect(normalized.reason).toBe("fresh scan");
  });
});
