import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  classifyBindError,
  missingBrowserPaths,
  parseBrowserInstallLocations,
  resolveRepositoryRoot,
} from "./verify-push-lib.mjs";

describe("push-gate port preflight", () => {
  it("distinguishes an occupied port from other bind failures", () => {
    expect(classifyBindError({ code: "EADDRINUSE" })).toBe("occupied");
    expect(classifyBindError({ code: "EACCES" })).toBe("unavailable");
    expect(classifyBindError(new Error("network unavailable"))).toBe("unavailable");
  });
});

describe("repository root resolution", () => {
  it("decodes spaces in checkout paths", () => {
    expect(
      resolveRepositoryRoot(
        "file:///Users/dev/My%20Projects/SiteCMD/tools/scripts/verify-push.mjs",
      ),
    ).toBe("/Users/dev/My Projects/SiteCMD");
  });
});

const DRY_RUN_OUTPUT = `Chrome for Testing 149.0.7827.55 (playwright chromium v1228)
  Install location:    /cache/ms-playwright/chromium-1228
  Download url:        https://cdn.playwright.dev/builds/cft/149.0.7827.55/mac-arm64/chrome-mac-arm64.zip

FFmpeg (playwright ffmpeg v1011)
  Install location:    /cache/ms-playwright/ffmpeg-1011
  Download url:        https://cdn.playwright.dev/builds/ffmpeg/1011/ffmpeg-mac-arm64.zip
  Download fallback 1: https://playwright.download.prss.microsoft.com/builds/ffmpeg/1011/ffmpeg-mac-arm64.zip

Chrome Headless Shell 149.0.7827.55 (playwright chromium-headless-shell v1228)
  Install location:    /cache/ms-playwright/chromium_headless_shell-1228
  Download url:        https://cdn.playwright.dev/builds/cft/149.0.7827.55/mac-arm64/chrome-headless-shell-mac-arm64.zip
`;

describe("playwright browser preflight", () => {
  it("reads every required install location, ignoring download URLs", () => {
    expect(parseBrowserInstallLocations(DRY_RUN_OUTPUT)).toEqual([
      "/cache/ms-playwright/chromium-1228",
      "/cache/ms-playwright/ffmpeg-1011",
      "/cache/ms-playwright/chromium_headless_shell-1228",
    ]);
  });

  it("passes when every browser is on disk", () => {
    expect(missingBrowserPaths(DRY_RUN_OUTPUT, () => true)).toEqual([]);
  });

  it("names only the browsers that are absent", () => {
    const present = new Set([
      "/cache/ms-playwright/chromium-1228",
      "/cache/ms-playwright/ffmpeg-1011",
    ]);
    expect(missingBrowserPaths(DRY_RUN_OUTPUT, (path) => present.has(path))).toEqual([
      "/cache/ms-playwright/chromium_headless_shell-1228",
    ]);
  });

  it("reports unparseable output instead of claiming everything is installed", () => {
    expect(missingBrowserPaths("Downloading Chromium...\ndone\n", () => false)).toBeNull();
    expect(missingBrowserPaths("", () => false)).toBeNull();
  });
});

describe("push-gate resource isolation", () => {
  it("does not run desktop UI tests beside the heavy Rust suite", () => {
    const source = readFileSync(
      fileURLToPath(new URL("./verify-push.mjs", import.meta.url)),
      "utf8",
    );
    const desktopTest = source.indexOf('name: "desktop-vitest"');
    expect(desktopTest).toBeGreaterThan(-1);
    const tierStart = source.lastIndexOf("  [", desktopTest);
    const tierEnd = source.indexOf("\n  ],", desktopTest);
    expect(tierStart).toBeGreaterThan(-1);
    expect(tierEnd).toBeGreaterThan(desktopTest);
    const desktopTier = source.slice(tierStart, tierEnd);
    expect(desktopTier).not.toContain('name: "rust-nextest"');
    expect(desktopTier).not.toContain('name: "rust-clippy"');
  });

  it("mirrors dependency policy and the declared Rust MSRV", () => {
    const source = readFileSync(
      fileURLToPath(new URL("./verify-push.mjs", import.meta.url)),
      "utf8",
    );
    const msrvWorkflow = readFileSync(
      fileURLToPath(new URL("../../.github/workflows/rust-msrv.yml", import.meta.url)),
      "utf8",
    );
    const msrv = /toolchain:\s*(\d+\.\d+\.\d+)/.exec(msrvWorkflow)?.[1];
    expect(msrv).toBeTruthy();
    expect(source).toContain('name: "audit:licenses:js"');
    expect(source).toContain('name: "cargo-deny"');
    expect(source).toContain(`cargo +${msrv} check --locked --workspace --all-targets`);
    expect(source).toContain(
      `cargo +${msrv} check --locked --manifest-path crates/cli/Cargo.toml --all-targets`,
    );
  });
});
