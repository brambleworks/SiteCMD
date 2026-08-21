import { describe, expect, it } from "vitest";
import {
  buildDeepLinkUrl,
  latestActivateDeepLinkKey,
  parseActivateDeepLink,
  parseDeepLinkUrl,
} from "./deep-links";
import type { AppTarget } from "./app-targets";

describe("parseDeepLinkUrl", () => {
  it("rejects non-sitecmd schemes", () => {
    expect(parseDeepLinkUrl("https://example.com?page=issues")).toBeNull();
    expect(parseDeepLinkUrl("mailto:foo@example.com")).toBeNull();
  });

  it("rejects invalid URLs", () => {
    expect(parseDeepLinkUrl("not a url")).toBeNull();
    expect(parseDeepLinkUrl("")).toBeNull();
  });

  it("parses a minimal sitecmd://open?page=dashboard", () => {
    const target = parseDeepLinkUrl("sitecmd://open?page=dashboard");
    expect(target?.page).toBe("dashboard");
    expect(target?.projectId).toBeNull();
  });

  it("parses project + page + scan context", () => {
    const target = parseDeepLinkUrl(
      "sitecmd://open?page=issues&projectId=5&scanId=12&sessionId=44&scanKind=code&url=https://example.com/",
    );
    expect(target?.page).toBe("issues");
    expect(target?.projectId).toBe(5);
    expect(target?.scanId).toBe(12);
    expect(target?.sessionId).toBe(44);
    expect(target?.scanKind).toBe("code");
    expect(target?.url).toBe("https://example.com"); // normalized
  });

  it("drops unsafe target URLs from deep links", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&url=javascript:alert(1)")?.url).toBeNull();
    expect(
      parseDeepLinkUrl("sitecmd://open?page=issues&url=https://user:token@example.com")?.url,
    ).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&url=https://example.com/foo/")?.url).toBe(
      "https://example.com/foo",
    );
  });

  it("rejects partial, negative, zero, and unsafe integer deep-link ids", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&projectId=7abc")?.projectId).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&scanId=12.5")?.scanId).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&sessionId=-1")?.sessionId).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&projectId=0")?.projectId).toBeNull();
    expect(
      parseDeepLinkUrl("sitecmd://open?page=issues&scanId=9007199254740993")?.scanId,
    ).toBeNull();
  });

  it("drops invalid scanKind values to null", () => {
    const target = parseDeepLinkUrl("sitecmd://open?page=issues&scanKind=invalid");
    expect(target?.scanKind).toBeNull();
  });

  it("parses focus + itemId + reason + filePath", () => {
    const target = parseDeepLinkUrl(
      "sitecmd://open?page=issues&focus=csp&itemId=issue-1&reason=fresh+scan&filePath=%2Ftmp%2Fapp.ts",
    );
    expect(target?.focus).toBe("csp");
    expect(target?.itemId).toBe("issue-1");
    expect(target?.reason).toBe("fresh scan");
    expect(target?.filePath).toBe("/tmp/app.ts");
  });

  it("drops oversized or control-character text params from deep links", () => {
    const oversized = "a".repeat(201);
    const target = parseDeepLinkUrl(
      `sitecmd://open?page=issues&focus=${oversized}&itemId=issue%0A1&reason=${encodeURIComponent("ok")}`,
    );

    expect(target?.focus).toBeNull();
    expect(target?.itemId).toBeNull();
    expect(target?.reason).toBe("ok");
  });

  it("recognizes lane=pending-verification and drops unknown lanes", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=updates&lane=pending-verification")?.lane).toBe(
      "pending-verification",
    );
    expect(parseDeepLinkUrl("sitecmd://open?page=updates&lane=mystery")?.lane).toBeNull();
  });

  it("treats restoreScan=1 and =true as truthy, everything else as false", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&restoreScan=1")?.restoreScan).toBe(true);
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&restoreScan=true")?.restoreScan).toBe(true);
    expect(parseDeepLinkUrl("sitecmd://open?page=issues&restoreScan=false")?.restoreScan).toBe(
      false,
    );
    expect(parseDeepLinkUrl("sitecmd://open?page=issues")?.restoreScan).toBe(false);
  });

  it("rejects URLs that don't resolve to any known page", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=nope")).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://nope")).toBeNull();
  });

  it("migrates the retired today page to sites", () => {
    expect(parseDeepLinkUrl("sitecmd://open?page=today")).toEqual({
      page: "sites",
      projectId: null,
      url: null,
      scanId: null,
      sessionId: null,
      scanKind: null,
      focus: null,
      itemId: null,
      promptId: null,
      lane: null,
      reason: null,
      filePath: null,
      restoreScan: false,
    });
  });

  it("parses page from the hostname slot when no ?page= is present", () => {
    const target = parseDeepLinkUrl("sitecmd://scans?scanId=3");
    expect(target?.page).toBe("issues");
    expect(target?.scanId).toBe(3);
  });

  it("parses project/:projectId/:page path form", () => {
    const target = parseDeepLinkUrl("sitecmd://project/42/issues");
    expect(target?.page).toBe("issues");
    expect(target?.projectId).toBe(42);
  });
});

describe("buildDeepLinkUrl round-trip", () => {
  function roundTrip(target: AppTarget): AppTarget | null {
    return parseDeepLinkUrl(buildDeepLinkUrl(target));
  }

  it("round-trips a minimal target", () => {
    const target: AppTarget = { page: "dashboard" };
    const parsed = roundTrip(target);
    expect(parsed?.page).toBe("dashboard");
  });

  it("round-trips a rich target without losing fields", () => {
    const target: AppTarget = {
      page: "issues",
      projectId: 7,
      url: "https://example.com/foo",
      scanId: 99,
      sessionId: 12,
      scanKind: "code",
      focus: "code-scan-domain:database",
      itemId: "db-owner-scope",
      promptId: "prompt-1",
      lane: "pending-verification",
      reason: "user fix",
      filePath: "/tmp/app/api/route.ts",
      restoreScan: true,
    };
    const parsed = roundTrip(target);
    expect(parsed).not.toBeNull();
    expect(parsed?.page).toBe("issues");
    expect(parsed?.projectId).toBe(7);
    expect(parsed?.url).toBe("https://example.com/foo");
    expect(parsed?.scanId).toBe(99);
    expect(parsed?.sessionId).toBe(12);
    expect(parsed?.scanKind).toBe("code");
    expect(parsed?.focus).toBe("code-scan-domain:database");
    expect(parsed?.itemId).toBe("db-owner-scope");
    expect(parsed?.promptId).toBe("prompt-1");
    expect(parsed?.lane).toBe("pending-verification");
    expect(parsed?.reason).toBe("user fix");
    expect(parsed?.filePath).toBe("/tmp/app/api/route.ts");
    expect(parsed?.restoreScan).toBe(true);
  });

  it("normalizes trailing slashes in the URL field on the way out", () => {
    const url = buildDeepLinkUrl({ page: "issues", url: "https://example.com/" });
    expect(url).toContain("url=https%3A%2F%2Fexample.com");
    expect(url).not.toContain("example.com%2F");
  });

  it("skips optional fields that are null/undefined rather than writing empty params", () => {
    const url = buildDeepLinkUrl({ page: "issues" });
    expect(url).not.toContain("projectId");
    expect(url).not.toContain("scanId");
    expect(url).not.toContain("sessionId");
    expect(url).not.toContain("focus");
  });
});

describe("parseActivateDeepLink", () => {
  it("parses a valid activation link", () => {
    expect(parseActivateDeepLink("sitecmd://activate?key=test-fixture-key-001")).toBe(
      "test-fixture-key-001", // gitleaks:allow
    );
  });

  it("url-decodes and trims the key, matching the Rust decoder", () => {
    expect(parseActivateDeepLink("sitecmd://activate?key=%20ABCD-1234%20")).toBe("ABCD-1234");
  });

  it("rejects empty or missing keys", () => {
    expect(parseActivateDeepLink("sitecmd://activate")).toBeNull();
    expect(parseActivateDeepLink("sitecmd://activate?key=")).toBeNull();
    expect(parseActivateDeepLink("sitecmd://activate?other=ABC")).toBeNull();
  });

  it("rejects wrong schemes, wrong hosts, and non-URLs", () => {
    expect(parseActivateDeepLink("https://sitecmd.com/activate?key=ABC")).toBeNull();
    expect(parseActivateDeepLink("sitecmd://open?key=ABC")).toBeNull();
    expect(parseActivateDeepLink("sitecmd://import?key=ABC")).toBeNull();
    expect(parseActivateDeepLink("not a url")).toBeNull();
  });

  it("rejects keys over the shared 256-character bound", () => {
    expect(parseActivateDeepLink(`sitecmd://activate?key=${"A".repeat(300)}`)).toBeNull();
  });
});

describe("latestActivateDeepLinkKey", () => {
  it("returns null for absent or empty startup URLs", () => {
    expect(latestActivateDeepLinkKey(null)).toBeNull();
    expect(latestActivateDeepLinkKey(undefined)).toBeNull();
    expect(latestActivateDeepLinkKey([])).toBeNull();
  });

  it("picks the last activation link, ignoring navigation links around it", () => {
    expect(
      latestActivateDeepLinkKey([
        "sitecmd://activate?key=OLD-KEY",
        "sitecmd://open?page=settings",
        "sitecmd://activate?key=NEW-KEY",
      ]),
    ).toBe("NEW-KEY");
  });

  it("returns null when no URL is an activation link", () => {
    expect(latestActivateDeepLinkKey(["sitecmd://open?page=dashboard"])).toBeNull();
  });
});

describe("connected deep links", () => {
  it("leaves the bare connected link to the refocus it has always been", () => {
    expect(parseDeepLinkUrl("sitecmd://connected")).toBeNull();
    expect(parseDeepLinkUrl("sitecmd://connected/")).toBeNull();
  });

  it("routes an alert link to the timeline carrying its opaque id", () => {
    const target = parseDeepLinkUrl("sitecmd://connected/alerts/alr_0123456789abcdef01234567");
    expect(target?.page).toBe("alerts");
    expect(target?.itemId).toBe("alr_0123456789abcdef01234567");
    expect(target?.reason).toBeNull();
  });

  it("routes both settings links to the connected settings tab", () => {
    for (const link of [
      "sitecmd://connected/settings/notifications",
      "sitecmd://connected/settings/admins",
    ]) {
      const target = parseDeepLinkUrl(link);
      expect(target?.page).toBe("settings");
      expect(target?.focus).toBe("connected");
    }
  });

  it("lands a malformed alert id on the not-found state without carrying it", () => {
    for (const hostile of [
      "sitecmd://connected/alerts/alr_1'%20OR%201=1",
      "sitecmd://connected/alerts/%2e%2e%2fetc%2fpasswd",
      "sitecmd://connected/alerts/<script>",
      "sitecmd://connected/alerts/",
      "sitecmd://connected/alerts",
      `sitecmd://connected/alerts/${"a".repeat(129)}`,
    ]) {
      const target = parseDeepLinkUrl(hostile);
      expect(target?.page).toBe("alerts");
      expect(target?.itemId).toBeNull();
      expect(target?.reason).toBe("connected-alert-unavailable");
    }
  });

  it("gives a connected path it has no page for a defined landing", () => {
    for (const unknown of [
      "sitecmd://connected/settings/whatever-ships-next",
      "sitecmd://connected/sites/site_1/notifications",
      "sitecmd://connected/reports/rep_1",
    ]) {
      const target = parseDeepLinkUrl(unknown);
      expect(target?.page).toBe("alerts");
      expect(target?.itemId).toBeNull();
      expect(target?.reason).toBe("connected-link-unknown");
    }
  });
});
