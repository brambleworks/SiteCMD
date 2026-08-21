import { describe, expect, it } from "vitest";
import { findUnifiedByCheckId, rankUnified } from "@/lib/issue-ranking";
import type { CheckResult } from "@/lib/types";

function makeWebCheck(checkId: string, overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    checkId: checkId,
    title: checkId,
    category: "security",
    severity: "high",
    status: "fail",
    description: "",
    ...overrides,
  } as CheckResult;
}

describe("IssuesPage navigation helpers", () => {
  it("finds a unified issue by web checkId", () => {
    const ranked = rankUnified(
      [makeWebCheck("security.hsts"), makeWebCheck("security.csp")],
      [],
      [],
      {},
    );
    const match = findUnifiedByCheckId(ranked, "security.hsts");
    expect(match?.kind).toBe("web");
    expect(match?.id).toBe("web:security.hsts");
  });

  it("returns null for a checkId not in the ranked list", () => {
    const ranked = rankUnified([makeWebCheck("security.csp")], [], [], {});
    expect(findUnifiedByCheckId(ranked, "security.hsts")).toBeNull();
  });

  it("caps stack depth at 5 when pushing", () => {
    const ranked = rankUnified(
      [
        makeWebCheck("a"),
        makeWebCheck("b"),
        makeWebCheck("c"),
        makeWebCheck("d"),
        makeWebCheck("e"),
        makeWebCheck("f"),
      ],
      [],
      [],
      {},
    );
    let stack: typeof ranked = [];
    for (const item of ranked) {
      stack = [...stack, item].slice(-5);
    }
    expect(stack).toHaveLength(5);
    expect(stack[0].id).toBe("web:b");
    expect(stack[4].id).toBe("web:f");
  });
});
