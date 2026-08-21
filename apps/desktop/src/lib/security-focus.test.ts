import { describe, expect, it } from "vitest";

import {
  getSecurityFocusLabel,
  getSecurityWatchImpactSentence,
  inferSecurityFocusFromText,
  matchesSecurityFocusText,
} from "./security-focus";

describe("security-focus", () => {
  it("returns labels for known focuses", () => {
    expect(getSecurityFocusLabel("sec.cors")).toBe("CORS and API boundary hardening");
    expect(getSecurityFocusLabel("sec.cookies")).toBe("Cookie and session security");
    expect(getSecurityFocusLabel("unknown.focus")).toBeNull();
  });

  it("matches known focus patterns against issue text", () => {
    expect(
      matchesSecurityFocusText(
        "security.cors Wildcard Access-Control-Allow-Origin with credentials",
        "sec.cors",
      ),
    ).toBe(true);
    expect(matchesSecurityFocusText("headers.csp Missing CSP header", "sec.cors")).toBe(false);
  });

  it("infers a focus from security issue text", () => {
    expect(inferSecurityFocusFromText("headers.hsts Missing HSTS header")).toBe("sec.hsts");
  });

  it("returns focus-aware watch impact copy with a safe fallback", () => {
    expect(getSecurityWatchImpactSentence("sec.auth")).toContain("authorization");
    expect(getSecurityWatchImpactSentence("unknown.focus")).toContain("security headers");
  });
});
