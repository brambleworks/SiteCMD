import { describe, expect, it } from "vitest";

import {
  getDesktopWatchImpactSentenceForReason,
  normalizeDesktopWatchReason,
} from "./desktop-watch-reasons";

describe("desktop-watch-reasons", () => {
  it("normalizes known watch kinds into semantic reasons", () => {
    expect(normalizeDesktopWatchReason("dependencies", "updates")).toBe("changed-dependencies");
    expect(normalizeDesktopWatchReason("robots", "search-console")).toBe("changed-search-file");
    expect(normalizeDesktopWatchReason("auth-guard", "issues")).toBe("changed-security-file");
    expect(normalizeDesktopWatchReason("unknown-kind", "issues")).toBe("unknown-kind");
  });

  it("returns shared focus-aware impact copy", () => {
    expect(
      getDesktopWatchImpactSentenceForReason({
        reason: "changed-search-file",
        page: "search-console",
      }),
    ).toContain("crawl directives");

    expect(
      getDesktopWatchImpactSentenceForReason({
        reason: "changed-security-file",
        page: "issues",
        focus: "sec.auth",
      }),
    ).toContain("authorization");
  });
});
