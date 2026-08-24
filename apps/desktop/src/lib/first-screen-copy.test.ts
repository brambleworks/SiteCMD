/// <reference types="node" />

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { getSecurityFocusLabel } from "./security-focus";

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relative: string) => readFileSync(path.join(SRC, relative), "utf8");

describe("first-screen copy stays in plain English", () => {
  it("does not describe the Issues page with internal vocabulary", () => {
    expect(read("app/ShellHeader.tsx")).not.toMatch(/paused fixes|regressions, paused/);
  });

  it("does not ask a new user to establish an issue baseline", () => {
    expect(read("components/scan/ScanHistory.tsx")).not.toContain("issue baseline");
  });

  it("explains what a site baseline is under the card title", () => {
    expect(read("components/dashboard/zones/SiteBaselineCard.tsx")).toContain(
      "What SiteCMD expects this site to keep doing.",
    );
  });

  it("names each web vital in words next to its acronym", () => {
    const source = read("components/dashboard/WebVitalsDetailModal.tsx");
    for (const plain of [
      "Largest content load",
      "Layout shift",
      "Main-thread blocking",
      "First content paint",
      "Server response time",
      "Visual completeness",
      "Input response",
    ]) {
      expect(source).toContain(plain);
    }
  });

  it("does not send a scan-config reader to loopback or Unix sockets", () => {
    expect(read("components/scan/ScanConfigOverlay.tsx")).not.toContain(
      "loopback or a local Unix socket",
    );
  });

  it("expands CSP and HSTS where they label a focus", () => {
    expect(getSecurityFocusLabel("sec.headers")).toBe("Content Security Policy (CSP) header");
    expect(getSecurityFocusLabel("sec.hsts")).toBe("HTTPS-only (HSTS) header");
  });
});
