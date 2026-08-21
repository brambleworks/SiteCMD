const PUBLIC_CLIENT_APPS = new Set(["desktop", "mcp-server"]);
const PRIVATE_STRATEGY_RECORD =
  /(?:^|\/)(?:publication-decision|paid-intelligence-rfc|commercial-terms-spec|connected-service-rfc)\.md$/;

/** Checks every path reachable from a proposed public ref. */
export function publicationHistoryPathFailures(paths) {
  const failures = [];
  const seen = new Set();
  const add = (failure) => {
    if (!seen.has(failure)) {
      seen.add(failure);
      failures.push(failure);
    }
  };

  for (const relativePath of paths) {
    if (PRIVATE_STRATEGY_RECORD.test(relativePath)) {
      add(`public history contains a private strategy record: ${relativePath}`);
    }

    const appMatch = relativePath.match(/^apps\/([^/]+)\//);
    if (appMatch && !PUBLIC_CLIENT_APPS.has(appMatch[1])) {
      add(`public history contains a private/non-client app tree: apps/${appMatch[1]}/`);
    }
  }

  return failures.sort();
}

export function candidateHistoryShapeFailures(commitCount, rootLine) {
  const failures = [];
  if (commitCount !== "1") {
    failures.push(`candidate main must contain exactly one commit (found ${commitCount})`);
  }
  if (rootLine.trim().split(/\s+/u).length !== 1) {
    failures.push("candidate main commit must have no parent");
  }
  return failures;
}
