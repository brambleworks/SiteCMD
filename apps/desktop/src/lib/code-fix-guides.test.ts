import { describe, expect, it } from "vitest";
import { getCodeFixGuide } from "./code-fix-guides";

describe("getCodeFixGuide", () => {
  it("returns null for unknown code scan checks", () => {
    expect(getCodeFixGuide("does-not-exist")).toBeNull();
  });

  it("returns exact producer-rule matches with effort metadata", () => {
    const guide = getCodeFixGuide("ai-timeout");
    expect(guide).not.toBeNull();
    expect(guide!.steps.length).toBeGreaterThan(0);
    expect(guide!.effortMinutes).toBeGreaterThan(0);
  });

  it("uses the explicit producer rule without parsing an occurrence ID", () => {
    const guide = getCodeFixGuide("ai-timeout");
    expect(guide).not.toBeNull();
    expect(guide!.steps.join("\n")).toContain("timeout");
  });
});
