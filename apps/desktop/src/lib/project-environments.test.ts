import { describe, expect, it } from "vitest";

import {
  getProjectUrlIdentityKey,
  inferProjectEnvironmentFromUrl,
  normalizeProjectUrlInput,
  resolveProjectEnvironmentForUrl,
} from "./project-environments";

describe("normalizeProjectUrlInput", () => {
  it("prefixes https:// onto bare domains and lowercases the host", () => {
    expect(normalizeProjectUrlInput("MySite.com")).toBe("https://mysite.com");
    expect(normalizeProjectUrlInput("example.com")).toBe("https://example.com");
  });

  it("lowercases a mixed-case scheme and host", () => {
    expect(normalizeProjectUrlInput("https://MySite.com")).toBe("https://mysite.com");
    expect(normalizeProjectUrlInput("HTTPS://SiteCMD.com")).toBe("https://sitecmd.com");
  });

  it("strips trailing slashes", () => {
    expect(normalizeProjectUrlInput("https://mysite.com/")).toBe("https://mysite.com");
    expect(normalizeProjectUrlInput("https://mysite.com//")).toBe("https://mysite.com");
  });

  it("preserves path case while lowercasing the origin (matches the Rust normalizer)", () => {
    expect(normalizeProjectUrlInput("https://Example.COM/About/")).toBe(
      "https://example.com/About",
    );
  });

  it("keeps ports and query strings intact", () => {
    expect(normalizeProjectUrlInput("http://Localhost:4321")).toBe("http://localhost:4321");
    expect(normalizeProjectUrlInput("https://Example.com/path?Q=Mixed")).toBe(
      "https://example.com/path?Q=Mixed",
    );
  });

  it("trims surrounding whitespace and returns empty for blank input", () => {
    expect(normalizeProjectUrlInput("  https://mysite.com  ")).toBe("https://mysite.com");
    expect(normalizeProjectUrlInput("   ")).toBe("");
  });
});

describe("getProjectUrlIdentityKey", () => {
  it("treats spelling variants of the same URL as one identity", () => {
    expect(getProjectUrlIdentityKey("https://MySite.com/")).toBe(
      getProjectUrlIdentityKey("mysite.com"),
    );
  });
});

describe("inferProjectEnvironmentFromUrl", () => {
  it("labels local dev environment hostnames as local", () => {
    // These resolve to loopback, so a detected DDEV/Lando/Docksal URL has to
    // land on Local rather than being read as another production site.
    expect(inferProjectEnvironmentFromUrl("https://smarthomeu.ddev.site")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("https://myapp.lndo.site")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://myapp.docksal.site")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://myapp.test")).toBe("local");
  });

  it("keeps a public host that merely contains a dev suffix on production", () => {
    expect(inferProjectEnvironmentFromUrl("https://ddev.site.example.com")).toBe("production");
  });

  it("labels a dev server on the local network as local", () => {
    // Rust grades these Local because a scan may reach them. If this surface
    // disagreed, the same URL would read Local in one place and Production in
    // the other. Mirrors `is_private_network_ip` in network_policy.rs.
    expect(inferProjectEnvironmentFromUrl("http://192.168.1.40:8080")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://10.0.0.5:3000")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://172.16.4.2")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://100.100.4.7")).toBe("local");
    expect(inferProjectEnvironmentFromUrl("http://[fd00::1]:8080")).toBe("local");
  });

  it("leaves a public address and a link-local address off local", () => {
    // Link-local is refused at the scan boundary rather than graded, and
    // 172.32 is outside the private block that 172.16-31 covers.
    expect(inferProjectEnvironmentFromUrl("http://93.184.216.34")).toBe("production");
    expect(inferProjectEnvironmentFromUrl("http://169.254.169.254")).toBe("production");
    expect(inferProjectEnvironmentFromUrl("http://172.32.0.1")).toBe("production");
  });
});

describe("resolveProjectEnvironmentForUrl", () => {
  it("keeps the detected local label for a DDEV URL", () => {
    expect(resolveProjectEnvironmentForUrl("https://smarthomeu.ddev.site", "local")).toBe("local");
  });

  it("still corrects a local label on a public host", () => {
    expect(resolveProjectEnvironmentForUrl("https://example.com", "local")).toBe("production");
  });
});
