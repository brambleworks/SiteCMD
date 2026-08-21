import { describe, expect, it } from "vitest";

import {
  cn,
  formatUrlDisplay,
  formatUrlHost,
  formatUrlHostPath,
  formatUrlPathOrHost,
  getHostname,
  getUrlPathname,
} from "./utils";

describe("cn", () => {
  it("merges simple class strings", () => {
    expect(cn("a", "b", "c")).toBe("a b c");
  });

  it("drops falsy values", () => {
    expect(cn("a", false, null, undefined, "b")).toBe("a b");
  });

  it("handles conditional object form", () => {
    expect(cn("base", { active: true, disabled: false })).toBe("base active");
  });

  it("joins all classes in order without conflict resolution", () => {
    expect(cn("one", "two")).toBe("one two");
    expect(cn("row-between", "commit-row")).toBe("row-between commit-row");
  });

  it("preserves every provided class alongside conditionals", () => {
    expect(cn("text-body font-bold flex-fill", "text-truncate")).toBe(
      "text-body font-bold flex-fill text-truncate",
    );
  });
});

describe("getHostname", () => {
  it("extracts hostname from a normal URL", () => {
    expect(getHostname("https://example.com/path")).toBe("example.com");
  });

  it("handles subdomains", () => {
    expect(getHostname("https://www.example.com")).toBe("www.example.com");
    expect(getHostname("https://staging.api.example.com/v2")).toBe("staging.api.example.com");
  });

  it("handles ports", () => {
    expect(getHostname("http://localhost:3000")).toBe("localhost");
    expect(getHostname("http://127.0.0.1:8080")).toBe("127.0.0.1");
  });

  it("returns an empty string for invalid URLs", () => {
    expect(getHostname("not a url")).toBe("");
    expect(getHostname("")).toBe("");
    expect(getHostname("   ")).toBe("");
  });

  it("handles URLs with query strings and fragments", () => {
    expect(getHostname("https://example.com/search?q=foo#results")).toBe("example.com");
  });
});

describe("getUrlPathname", () => {
  it("extracts a display path from a URL", () => {
    expect(getUrlPathname("https://example.com/docs/page?token=secret#section")).toBe("/docs/page");
  });

  it("returns slash for URL roots", () => {
    expect(getUrlPathname("https://example.com")).toBe("/");
  });

  it("returns the provided fallback for malformed or missing URLs", () => {
    expect(getUrlPathname("not a url", "https://example.com")).toBe("https://example.com");
    expect(getUrlPathname(null, "fallback")).toBe("fallback");
    expect(getUrlPathname(undefined, "fallback")).toBe("fallback");
  });
});

describe("url display helpers", () => {
  it("formats affected URL labels as path when useful and host otherwise", () => {
    expect(formatUrlPathOrHost("https://example.com/docs/page?token=secret")).toBe("/docs/page");
    expect(formatUrlPathOrHost("https://example.com/")).toBe("example.com");
    expect(formatUrlPathOrHost("/already-a-path")).toBe("/already-a-path");
    expect(formatUrlPathOrHost(null, "current site")).toBe("current site");
  });

  it("formats event URL labels as host plus path without query secrets", () => {
    expect(formatUrlHostPath("https://example.com/docs/page?token=secret#section")).toBe(
      "example.com/docs/page",
    );
    expect(formatUrlHostPath("https://example.com/")).toBe("example.com");
    expect(formatUrlHostPath("not a url?token=secret")).toBe("not a url");
  });

  it("formats compact URL labels without losing path context", () => {
    expect(formatUrlDisplay("https://example.com/docs/?q=launch")).toBe(
      "example.com/docs/?q=launch",
    );
    expect(formatUrlDisplay("http://localhost:5173/")).toBe("localhost:5173");
  });

  it("extracts host labels from valid and partial URLs", () => {
    expect(formatUrlHost("https://example.com/docs/?q=launch")).toBe("example.com");
    expect(formatUrlHost("localhost:5173/docs")).toBe("localhost:5173");
    expect(formatUrlHost(null, "current site")).toBe("current site");
  });
});
