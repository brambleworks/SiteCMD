import { describe, expect, it } from "vitest";

import manifest from "@/generated/issue_status.json";
import { INACTIVE_ISSUE_STATUSES } from "@/pages/issues/active-issue-filter";
import type { IssueStatus } from "@/lib/types";

const TS_UNION_MEMBERS: Record<IssueStatus, true> = {
  new: true,
  snoozed: true,
  ignored: true,
  blocked: true,
  verified: true,
  regressed: true,
};

describe("issue status vocabulary parity", () => {
  it("matches the generated Rust manifest exactly", () => {
    expect(Object.keys(TS_UNION_MEMBERS).sort()).toEqual([...manifest.statuses].sort());
  });

  it("keeps the active-issue filter a subset of the Rust inactive set", () => {
    const rustInactive = new Set(manifest.inactive_for_scoring);
    for (const status of INACTIVE_ISSUE_STATUSES) {
      expect(rustInactive.has(status), `${status} must be inactive in Rust too`).toBe(true);
    }
    const unmirrored = manifest.inactive_for_scoring.filter(
      (status) => !INACTIVE_ISSUE_STATUSES.includes(status as IssueStatus),
    );
    expect(unmirrored).toEqual(["snoozed"]);
  });

  it("keeps regressed active so re-failed issues count again", () => {
    expect(manifest.inactive_for_scoring).not.toContain("regressed");
  });
});
