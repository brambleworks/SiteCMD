import {
  requireCondition,
  requireNumber,
  requireText,
  validateSubscriptionBilling,
} from "./workflow-contract.mjs";

function timestamp(value, label) {
  requireCondition(
    typeof value === "string" && /Z$/.test(value),
    `${label} must be a UTC timestamp`,
  );
  const parsed = Date.parse(value);
  requireCondition(Number.isFinite(parsed), `${label} must be a UTC timestamp`);
  return parsed;
}

function validateSnapshot(snapshot) {
  requireCondition(snapshot?.schemaVersion === 1, "quota schemaVersion must be 1");
  const capturedAt = timestamp(snapshot.capturedAt, "capturedAt");
  requireText(snapshot.source, "quota evidence source");
  requireCondition(Array.isArray(snapshot.accounts), "quota accounts are required");
  requireCondition(
    JSON.stringify(snapshot.accounts.map((item) => item.provider).sort()) === '["claude","codex"]',
    "quota requires exactly one Codex and one Claude account",
  );
  for (const account of snapshot.accounts) {
    requireText(account.account, "stable account label");
    requireCondition(
      Array.isArray(account.windows) && account.windows.length > 0,
      "quota windows are required",
    );
    const ids = new Set();
    for (const window of account.windows) {
      requireText(window.id, "quota window id");
      requireCondition(!ids.has(window.id), "duplicate quota window");
      ids.add(window.id);
      requireCondition(["weekly", "session"].includes(window.kind), "invalid quota window kind");
      if (window.usedPercent !== null) {
        requireNumber(window.usedPercent, "usedPercent");
        requireCondition(window.usedPercent <= 100, "usedPercent must not exceed 100");
      }
      if (window.resetsAt !== null) timestamp(window.resetsAt, "resetsAt");
    }
    requireCondition(
      account.windows.some((window) => window.kind === "weekly"),
      "weekly quota is required",
    );
  }
  return capturedAt;
}

function checkWindow(before, current, label, policy, capturedAt, now, blockers) {
  if (before.kind !== current.kind) blockers.push(`${label}: quota window kind changed`);
  if (current.usedPercent === null || before.usedPercent === null) {
    blockers.push(`${label}: usage is unknown`);
    return;
  }
  const reset = current.resetsAt === null ? NaN : timestamp(current.resetsAt, "resetsAt");
  const previousReset = before.resetsAt === null ? NaN : timestamp(before.resetsAt, "resetsAt");
  if (!Number.isFinite(reset) || !Number.isFinite(previousReset) || reset <= now) {
    blockers.push(`${label}: reset time is unknown or the window has expired`);
    return;
  }
  const sameWindow = before.resetsAt === current.resetsAt;
  if (
    !sameWindow &&
    (current.kind === "weekly" || capturedAt < previousReset || reset <= previousReset)
  )
    blockers.push(`${label}: reset changed; do not silently rebase the approved budget`);
  if (sameWindow && current.usedPercent < before.usedPercent)
    blockers.push(`${label}: usage decreased within the same window`);
  if (100 - current.usedPercent < policy.minimumRemainingPercent)
    blockers.push(`${label}: less than ${policy.minimumRemainingPercent}% remaining`);
  if (
    current.kind === "weekly" &&
    current.usedPercent - before.usedPercent >= policy.weeklyBudgetPercentagePoints
  )
    blockers.push(
      `${label}: ${policy.weeklyBudgetPercentagePoints} percentage points of weekly allowance consumed`,
    );
}

/** Evaluate supplied account readings; this does not fetch quotas or stop an agent process. */
export function evaluateQuota(baseline, current, policy, now = Date.now()) {
  validateSubscriptionBilling(policy);
  const baselineAt = validateSnapshot(baseline);
  const capturedAt = validateSnapshot(current);
  requireNumber(now, "current time");
  const blockers = [];
  if (capturedAt < baselineAt || baselineAt > now || capturedAt > now)
    blockers.push("Quota evidence timestamps are out of order or in the future");
  if (now - capturedAt > policy.quotaMaxAgeSeconds * 1000)
    blockers.push("Current quota evidence is stale");
  for (const account of current.accounts) {
    const before = baseline.accounts.find((item) => item.provider === account.provider);
    const label = account.provider;
    if (before.account !== account.account) blockers.push(`${label}: account changed`);
    if (before.authMode !== "subscription" || account.authMode !== "subscription")
      blockers.push(`${label}: subscription authentication is unverified`);
    if (before.extraUsageEnabled !== false || account.extraUsageEnabled !== false)
      blockers.push(`${label}: additional paid usage must be verified disabled`);
    const ids = (item) =>
      item.windows
        .map((window) => window.id)
        .sort()
        .join("\n");
    if (ids(before) !== ids(account)) {
      blockers.push(`${label}: quota windows changed or are missing`);
      continue;
    }
    for (const window of account.windows) {
      const previous = before.windows.find((item) => item.id === window.id);
      if (previous.resetsAt !== null && timestamp(previous.resetsAt, "resetsAt") <= baselineAt)
        blockers.push(`${label}/${window.id}: baseline window had already expired`);
      checkWindow(previous, window, `${label}/${window.id}`, policy, capturedAt, now, blockers);
    }
  }
  return { quotaAllowed: blockers.length === 0, blockers, capturedAt: current.capturedAt };
}
