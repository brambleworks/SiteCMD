import assert from "node:assert/strict";
import { test } from "node:test";
import { evaluateQuota } from "./workflow-quota.mjs";
import { pilotPolicy } from "./workflow-pilot.mjs";

const NOW = Date.parse("2026-09-03T16:00:00Z");
function snapshot(capturedAt = "2026-09-03T16:00:00Z") {
  return {
    schemaVersion: 1,
    capturedAt,
    source: "Test fixture, not an account reading",
    accounts: ["codex", "claude"].map((provider) => ({
      provider,
      account: `${provider}-test-account`,
      authMode: "subscription",
      extraUsageEnabled: false,
      windows: [
        { id: "weekly", kind: "weekly", usedPercent: 10, resetsAt: "2026-09-07T00:00:00Z" },
        { id: "session", kind: "session", usedPercent: 20, resetsAt: "2026-09-03T18:00:00Z" },
      ],
    })),
  };
}

test("the pilot allows only the approved assignments, exact models, and no additional spending", () => {
  assert.equal(
    pilotPolicy.caseCount *
      pilotPolicy.models.length *
      pilotPolicy.arms.length *
      pilotPolicy.repeats,
    45,
  );
  assert.deepEqual(
    pilotPolicy.models.map((item) => item.model),
    ["gpt-5.6-sol", "claude-opus-5", "claude-fable-5-1"],
  );
  assert.equal(pilotPolicy.limits.studyCostUsd, 0);
  assert.equal(pilotPolicy.limits.trialSeconds, 1200);
  assert.equal(pilotPolicy.limits.submissions, 3);
});

test("quota checks both subscriptions and pauses when either weekly allocation is consumed", () => {
  const baseline = snapshot("2026-09-03T15:00:00Z");
  const current = snapshot();
  const check = () => evaluateQuota(baseline, current, pilotPolicy.billing, NOW);
  assert.equal(check().quotaAllowed, true);
  current.accounts[1].windows[0].usedPercent = 30;
  assert.equal(check().quotaAllowed, false);
  assert.match(check().blockers.join("\n"), /claude.*20 percentage points/);
  current.accounts[1].windows[0].usedPercent = 29;
  current.accounts[0].windows[1].usedPercent = 71;
  assert.match(check().blockers.join("\n"), /codex.*30%/);
});

test("unknown, stale, reset, and differently authenticated quotas cannot authorize a trial", () => {
  for (const [change, message] of [
    [
      (item) => {
        item.capturedAt = "2026-09-03T15:54:59Z";
      },
      /stale/,
    ],
    [
      (item) => {
        item.capturedAt = "2026-09-03T16:00:01Z";
      },
      /future/,
    ],
    [
      (item) => {
        item.accounts[0].windows[0].usedPercent = null;
      },
      /unknown/,
    ],
    [
      (item) => {
        item.accounts[0].windows[0].resetsAt = null;
      },
      /unknown/,
    ],
    [
      (item) => {
        item.accounts[0].windows[0].resetsAt = "2026-09-14T00:00:00Z";
      },
      /rebase/,
    ],
    [
      (item) => {
        item.accounts[0].windows[0].usedPercent = 9;
      },
      /decreased/,
    ],
    [
      (item) => {
        item.accounts[0].account = "replacement-account";
      },
      /account changed/,
    ],
    [
      (item) => {
        item.accounts[1].authMode = "api";
      },
      /authentication/,
    ],
    [
      (item) => {
        item.accounts[1].extraUsageEnabled = null;
      },
      /verified disabled/,
    ],
    [
      (item) => {
        item.accounts[1].extraUsageEnabled = true;
      },
      /verified disabled/,
    ],
    [
      (item) => {
        item.accounts[0].windows.pop();
      },
      /missing/,
    ],
  ]) {
    const baseline = snapshot("2026-09-03T15:00:00Z");
    const current = snapshot();
    change(current);
    const result = evaluateQuota(baseline, current, pilotPolicy.billing, NOW);
    assert.equal(result.quotaAllowed, false);
    assert.match(result.blockers.join("\n"), message);
  }
});

test("malformed or missing accounts and windows fail closed", () => {
  for (const change of [
    (item) => {
      item.accounts.pop();
    },
    (item) => {
      item.accounts[1] = item.accounts[0];
    },
    (item) => {
      item.accounts[0].windows.pop();
      item.accounts[0].windows[0].kind = "session";
    },
    (item) => {
      item.accounts[0].windows[0].usedPercent = 101;
    },
    (item) => {
      delete item.accounts[0].windows[0].usedPercent;
    },
    (item) => {
      item.accounts[0].windows.push(item.accounts[0].windows[0]);
    },
  ]) {
    const current = snapshot();
    change(current);
    assert.throws(() => evaluateQuota(snapshot(), current, pilotPolicy.billing, NOW));
  }
});

test("a real session reset does not replenish the frozen weekly budget", () => {
  const baseline = snapshot("2026-09-03T15:00:00Z");
  const current = snapshot("2026-09-03T18:01:00Z");
  for (const account of current.accounts) {
    account.windows[1].usedPercent = 0;
    account.windows[1].resetsAt = "2026-09-03T23:00:00Z";
  }
  const later = Date.parse(current.capturedAt);
  assert.equal(evaluateQuota(baseline, current, pilotPolicy.billing, later).quotaAllowed, true);
  current.accounts[0].windows[0].usedPercent = 30;
  assert.equal(evaluateQuota(baseline, current, pilotPolicy.billing, later).quotaAllowed, false);
});
