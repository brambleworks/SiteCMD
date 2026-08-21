import { describe, expect, it, vi } from "vitest";

// Tauri IPC bridge is referenced at module load inside UpdatesPage.
// Mock it up-front so importing the file doesn't blow up.
vi.mock("@/lib/tauri-invoke", () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/lib/store", () => ({
  storeSet: vi.fn(() => Promise.resolve()),
  storeGet: vi.fn(() => Promise.resolve(null)),
  migrateFromLocalStorage: vi.fn(() => Promise.resolve(null)),
}));

import { buildAiTask, buildCommand, buildUpdateRefreshHistoryDraft } from "./UpdatesPage";
import type { PackageUpdate } from "@/lib/types";

function update(overrides: Partial<PackageUpdate> = {}): PackageUpdate {
  return {
    ecosystem: "npm",
    name: "react",
    currentVersion: "18.2.0",
    latestVersion: "19.0.0",
    updateType: "major",
    isSecurity: false,
    ...overrides,
  } as PackageUpdate;
}

describe("buildCommand", () => {
  it("npm install for npm ecosystem", () => {
    expect(buildCommand(update({ ecosystem: "npm", name: "lodash", latestVersion: "5.0.0" }))).toBe(
      "npm install lodash@5.0.0",
    );
  });

  it("composer require for composer ecosystem", () => {
    expect(
      buildCommand(
        update({ ecosystem: "composer", name: "guzzlehttp/guzzle", latestVersion: "7.8.0" }),
      ),
    ).toBe("composer require guzzlehttp/guzzle:7.8.0");
  });

  it("wp plugin update for wordpress plugins (version is WP-managed)", () => {
    expect(buildCommand(update({ ecosystem: "wordpress", name: "woocommerce" }))).toBe(
      "wp plugin update woocommerce",
    );
  });

  it("drupal prefixes the vendor with drupal/", () => {
    expect(
      buildCommand(update({ ecosystem: "drupal", name: "views", latestVersion: "10.1.0" })),
    ).toBe("composer require drupal/views:10.1.0");
  });

  it("pip install for python ecosystem", () => {
    expect(
      buildCommand(update({ ecosystem: "python", name: "requests", latestVersion: "2.32.0" })),
    ).toBe("pip install requests==2.32.0");
  });

  it("bundle update for ruby", () => {
    expect(buildCommand(update({ ecosystem: "ruby", name: "rails" }))).toBe("bundle update rails");
  });

  it("go get pins to 'vX.Y.Z' tag", () => {
    expect(
      buildCommand(
        update({ ecosystem: "go", name: "github.com/gin-gonic/gin", latestVersion: "1.9.1" }),
      ),
    ).toBe("go get github.com/gin-gonic/gin@v1.9.1");
  });

  it("cargo update for rust (version is cargo-managed)", () => {
    expect(buildCommand(update({ ecosystem: "rust", name: "serde" }))).toBe(
      "cargo update -p serde",
    );
  });
});

describe("buildAiTask", () => {
  it("includes the package name in the opening line", () => {
    const out = buildAiTask(update({ name: "lodash" }));
    expect(out).toContain("Help me update lodash");
  });

  it("tags non-security updates with their update_type", () => {
    const out = buildAiTask(update({ updateType: "minor", isSecurity: false }));
    expect(out).toContain("minor dependency update");
    expect(out).not.toContain("security update");
  });

  it("security updates include the advisory severity", () => {
    const out = buildAiTask(
      update({
        isSecurity: true,
        advisorySeverity: "critical",
        advisoryFixedVersion: "19.0.0",
      }),
    );
    expect(out).toContain("security update");
    expect(out).toContain("critical");
  });

  it("security updates without advisory_severity fall back to 'unknown'", () => {
    const out = buildAiTask(update({ isSecurity: true, advisoryFixedVersion: "19.0.0" }));
    expect(out).toContain("severity unknown");
  });

  it("does not invent an upgrade command when no fixed release is published", () => {
    const vulnerable = update({ isSecurity: true, advisorySeverity: "high" });
    const out = buildAiTask(vulnerable);

    expect(buildCommand(vulnerable)).toBeNull();
    expect(out).toContain("Fixed release: not published");
    expect(out).toContain("recommend bounded mitigations or a replacement package");
    expect(out).not.toContain("Target version:");
  });

  it("includes current and target version lines", () => {
    const out = buildAiTask(update({ currentVersion: "1.0.0", latestVersion: "2.0.0" }));
    expect(out).toContain("Current version: 1.0.0");
    expect(out).toContain("Target version: 2.0.0");
  });

  it("translates ecosystem keys to human labels", () => {
    expect(buildAiTask(update({ ecosystem: "npm" }))).toContain("Ecosystem: npm");
    expect(buildAiTask(update({ ecosystem: "python" }))).toContain("Ecosystem: Python");
    expect(buildAiTask(update({ ecosystem: "rust" }))).toContain("Ecosystem: Rust");
  });

  it("includes the canonical 'Please' follow-up instructions", () => {
    const out = buildAiTask(update());
    expect(out).toMatch(/safest upgrade path/);
    expect(out).toMatch(/breaking changes/);
    expect(out).toMatch(/verification steps/);
  });
});

describe("buildUpdateRefreshHistoryDraft", () => {
  it("summarizes when updates are cleared after a refresh", () => {
    const previous = [
      update({ name: "astro" }),
      update({ name: "react-dom", updateType: "minor", latestVersion: "18.3.0" }),
    ];
    const next = [update({ name: "react-dom", updateType: "minor", latestVersion: "18.3.0" })];

    const draft = buildUpdateRefreshHistoryDraft(previous, next);

    expect(draft).not.toBeNull();
    expect(draft?.title).toBe("1 Update Applied");
    expect(draft?.summary).toContain("astro 18.2.0 -> 19.0.0 • major left the list.");
    expect(draft?.detail.verified_count).toBe(1);
    expect(draft?.detail.remaining_updates).toBe(1);
    expect(draft?.detail.applied_updates).toEqual([
      {
        name: "astro",
        from_version: "18.2.0",
        to_version: "19.0.0",
      },
    ]);
  });

  it("names multiple cleared packages in the history summary", () => {
    const previous = [
      update({
        name: "@tailwindcss/vite",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
      update({
        name: "tailwindcss",
        currentVersion: "4.1.13",
        latestVersion: "4.2.2",
        updateType: "minor",
      }),
    ];

    const draft = buildUpdateRefreshHistoryDraft(previous, []);

    expect(draft).not.toBeNull();
    expect(draft?.summary).toContain("@tailwindcss/vite 4.1.13 -> 4.2.2 • minor");
    expect(draft?.summary).toContain("tailwindcss 4.1.13 -> 4.2.2 • minor");
    expect(draft?.detail.remaining_updates).toBe(0);
  });

  it("does not create history entries for newly discovered pending updates", () => {
    const next = [update({ name: "astro" })];
    expect(buildUpdateRefreshHistoryDraft([], next)).toBeNull();
  });

  it("records a cleared no-fix advisory as resolved without inventing a target version", () => {
    const vulnerable = update({
      name: "lodash",
      currentVersion: "4.17.20",
      latestVersion: "4.17.21",
      isSecurity: true,
      advisorySeverity: "high",
    });

    const draft = buildUpdateRefreshHistoryDraft([vulnerable], []);

    expect(draft?.summary).toContain("lodash 4.17.20 (no fixed release) • security (high)");
    expect(draft?.detail.applied_updates).toEqual([
      { name: "lodash", from_version: "4.17.20", to_version: "resolved" },
    ]);
  });

  it("returns null when the update queue has not changed", () => {
    const previous = [update({ name: "astro" })];
    const next = [update({ name: "astro" })];

    expect(buildUpdateRefreshHistoryDraft(previous, next)).toBeNull();
  });
});
