import { describe, expect, it } from "vitest";
import {
  ISSUE_TRIAGE_COPY,
  TRIAGE_SCORE_RECOVERY_NOTE,
  type TriageActionCopy,
} from "./issue-triage-copy";

describe("issue triage copy", () => {
  it("describes Ignore as temporary until the next scan", () => {
    expect(ISSUE_TRIAGE_COPY.ignore.help).toMatch(/temporarily/i);
    expect(ISSUE_TRIAGE_COPY.ignore.help).toMatch(/next scan/i);
    expect(ISSUE_TRIAGE_COPY.ignore.help).toMatch(/returns to your active list/i);
  });

  it("labels the block lever and spells out its will-not-fix meaning", () => {
    expect(ISSUE_TRIAGE_COPY.block.label).toBe("Block");
    expect(ISSUE_TRIAGE_COPY.block.help).toMatch(/permanently/i);
    expect(ISSUE_TRIAGE_COPY.block.help).toMatch(/future scans/i);
    expect(ISSUE_TRIAGE_COPY.block.help).toMatch(/until you restore/i);
  });

  it("makes each action's score behavior explicit and keeps snooze distinct", () => {
    expect(ISSUE_TRIAGE_COPY.ignore.help).toMatch(/counts against your score/i);
    expect(ISSUE_TRIAGE_COPY.block.help).toMatch(/stays out of your active list and score/i);
    expect(ISSUE_TRIAGE_COPY.snooze.help).toMatch(/temporarily|returns to your active list/i);
  });

  it("carries a shared note that contrasts next-scan Ignore with durable Block", () => {
    expect(TRIAGE_SCORE_RECOVERY_NOTE).toMatch(/counts only active issues/i);
    expect(TRIAGE_SCORE_RECOVERY_NOTE).toMatch(/next scan/i);
    expect(TRIAGE_SCORE_RECOVERY_NOTE).toMatch(/future scans/i);
    expect(TRIAGE_SCORE_RECOVERY_NOTE).toMatch(/until you restore/i);
  });

  it("gives every action a non-empty label and help string", () => {
    const entries: TriageActionCopy[] = Object.values(ISSUE_TRIAGE_COPY);
    expect(entries).toHaveLength(3);
    for (const entry of entries) {
      expect(entry.label.trim().length).toBeGreaterThan(0);
      expect(entry.help.trim().length).toBeGreaterThan(0);
    }
  });

  it("never uses an em-dash (repo copy rule)", () => {
    const allCopy = [
      TRIAGE_SCORE_RECOVERY_NOTE,
      ...Object.values(ISSUE_TRIAGE_COPY).flatMap((entry) => [entry.label, entry.help]),
    ];
    for (const text of allCopy) {
      expect(text).not.toContain("—"); // allow-em-dash
    }
  });
});
