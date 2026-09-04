import assert from "node:assert/strict";
import { test } from "node:test";
import { closingQuota } from "../guest/closing-quota.mjs";
import { pilotPolicy } from "./workflow-pilot.mjs";

test("a pre-trial reading cannot stand in for post-trial quota evidence", async () => {
  const events = [];
  let time = 10000;
  const result = await closingQuota({
    baseline: {},
    currentPath: "fixture",
    billing: { quotaMaxAgeSeconds: 2 },
    endedAt: 9000,
    log: (...args) => events.push(args),
    now: () => time,
    wait: async () => {
      time += 1000;
    },
    read: () => ({ capturedAt: new Date(8000).toISOString() }),
  });
  assert.equal(result.quotaAllowed, false);
  assert.equal(time, 12000);
  assert.equal(events.length, 1);
});

test("fresh post-trial evidence is checked against the unchanged allowance baseline", async () => {
  const now = Date.now();
  const baseline = {
    schemaVersion: 1,
    capturedAt: new Date(now - 1000).toISOString(),
    source: "Unit fixture, not provider evidence",
    accounts: ["codex", "claude"].map((provider) => ({
      provider,
      account: `${provider}-fixture`,
      authMode: "subscription",
      extraUsageEnabled: false,
      windows: [
        {
          id: "weekly",
          kind: "weekly",
          usedPercent: 10,
          resetsAt: new Date(now + 86400000).toISOString(),
        },
      ],
    })),
  };
  const current = structuredClone(baseline);
  current.capturedAt = new Date(now).toISOString();
  current.accounts[1].windows[0].usedPercent = 30;
  const result = await closingQuota({
    baseline,
    currentPath: "fixture",
    billing: pilotPolicy.billing,
    endedAt: now - 500,
    log: () => {},
    now: () => now,
    read: () => current,
  });
  assert.equal(result.quotaAllowed, false);
  assert.match(result.blockers.join(" "), /20 percentage points/);
});
