import { describe, expect, it } from "vitest";
import { buildBatchFixPrompt, type BatchFixItem } from "./fix-copilot-batch";

function makeItem(overrides: Partial<BatchFixItem> = {}): BatchFixItem {
  return {
    kind: "web",
    title: "Content Security Policy is missing",
    severity: "high",
    category: "security",
    description: "Responses do not include a CSP header.",
    fixHint: null,
    filePath: null,
    ...overrides,
  };
}

describe("buildBatchFixPrompt", () => {
  it("groups web and code issues into separate sections with the total count", () => {
    const prompt = buildBatchFixPrompt(
      [
        makeItem({ fixHint: "Add a CSP header." }),
        makeItem({
          kind: "code",
          title: "Secret committed to the repo",
          severity: "critical",
          category: "security",
          description: "An API key is checked in.",
          filePath: "src/config.ts",
        }),
      ],
      { url: "https://example.com", detectedStack: { framework: "Astro" } },
    );

    expect(prompt).toContain("Fix the following 2 issues on https://example.com.");
    expect(prompt).toContain('Detected stack: {\n  "framework": "Astro"\n}');
    expect(prompt).toContain("## Web Scan Issues (1)");
    expect(prompt).toContain("### 1. Content Security Policy is missing (high)");
    expect(prompt).toContain("Fix direction: Add a CSP header.");
    expect(prompt).toContain("## Code Scan Issues (1)");
    expect(prompt).toContain("File: src/config.ts");
    expect(prompt).toContain("Keep each fix scoped and independent");
  });

  it("omits empty sections and the stack line when absent", () => {
    const prompt = buildBatchFixPrompt([makeItem()], {});

    expect(prompt).toContain("Fix the following 1 issue.");
    expect(prompt).not.toContain("Detected stack");
    expect(prompt).not.toContain("## Code Scan Issues");
  });
});
