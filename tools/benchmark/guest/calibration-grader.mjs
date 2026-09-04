import { isDeepStrictEqual } from "node:util";
import { executeCandidate } from "./candidate-sandbox.mjs";

export function gradeCase(item, candidate, execute = executeCandidate) {
  const checks = { acceptance: [], regressions: [] };
  const check = (group, name, input, predicate) => {
    const actual = execute(item, candidate, input);
    let passed;
    try {
      passed = predicate(actual);
    } catch {
      passed = false;
    }
    checks[group].push({ name, passed, actual });
  };
  if (item.id === "credentialed-cors") {
    const response = (origin) => ({
      url: "https://api.example.com/account",
      headers: origin ? { origin } : {},
    });
    for (const origin of [
      "https://evil.example",
      "null",
      "https://dashboard.example.com.evil.example",
      null,
    ])
      check(
        "acceptance",
        `Reject origin ${origin}`,
        response(origin),
        (out) => !out.error && out.headers["access-control-allow-origin"] === undefined,
      );
    for (const origin of ["https://dashboard.example.com", "https://admin.example.com"])
      check(
        "regressions",
        `Preserve origin ${origin}`,
        response(origin),
        (out) =>
          out.status === 200 &&
          out.headers["access-control-allow-origin"] === origin &&
          out.headers["access-control-allow-credentials"] === "true" &&
          isDeepStrictEqual(JSON.parse(out.body), { account: "demo", plan: "free" }),
      );
  } else if (item.id === "redirect-destination") {
    const response = (next) => ({
      url: `https://app.example.com/continue?next=${encodeURIComponent(next)}`,
    });
    for (const next of [
      "https://evil.example",
      "//evil.example",
      "/\\evil.example",
      "javascript:alert(1)",
      "\\\\evil.example",
    ])
      check(
        "acceptance",
        `Reject destination ${next}`,
        response(next),
        (out) => out.status === 302 && out.headers.location === "https://app.example.com/dashboard",
      );
    for (const next of ["/settings", "/reports?year=2026#total", "/dashboard"])
      check(
        "regressions",
        `Preserve destination ${next}`,
        response(next),
        (out) => out.status === 302 && out.headers.location === `https://app.example.com${next}`,
      );
    check(
      "regressions",
      "Default destination",
      { url: "https://app.example.com/continue" },
      (out) => out.status === 302 && out.headers.location === "https://app.example.com/dashboard",
    );
  } else if (item.id === "document-download") {
    const input = (supplied) => ({
      operation: "download",
      args: { path: supplied },
      text: "Example document 726",
    });
    for (const supplied of [
      "../outside.txt",
      "nested/../../outside.txt",
      "/tmp/outside.txt",
      "escape",
    ])
      check("acceptance", `Reject document ${supplied}`, input(supplied), (out) =>
        ["ValueError", "PermissionError"].includes(out.error),
      );
    check(
      "regressions",
      "Read document",
      input("readme.txt"),
      (out) => out.result === "Example document 726",
    );
    check(
      "regressions",
      "Read nested document",
      input("nested/guide.txt"),
      (out) => out.result === "Example document 726 nested",
    );
    check(
      "regressions",
      "Missing document",
      input("missing.txt"),
      (out) => out.error === "FileNotFoundError",
    );
  } else if (["user-lookup-query", "bound-user-lookup"].includes(item.id)) {
    const users = [
      [1, "alice"],
      [2, "bob"],
      [3, "O'Hare"],
    ];
    const lookup = (group, name, expected) =>
      check(
        group,
        `Look up ${name}`,
        { operation: "lookup", args: { name }, users },
        (out) => isDeepStrictEqual(out.result, expected) && isDeepStrictEqual(out.remaining, users),
      );
    lookup("acceptance", "' OR 1=1 --", []);
    lookup("acceptance", "O'Hare", [[3, "O'Hare"]]);
    lookup("acceptance", "alice'; DELETE FROM users; --", []);
    lookup("regressions", "alice", [[1, "alice"]]);
    lookup("regressions", "bob", [[2, "bob"]]);
    lookup("regressions", "nobody", []);
  } else {
    throw new Error(`No independent grader for case ${item.id}`);
  }
  check(
    "regressions",
    "Existing project tests",
    { operation: "public-tests" },
    (out) => out.exitCode === 0,
  );
  return {
    ...checks,
    acceptancePass: checks.acceptance.every((item) => item.passed),
    regressionsPass: checks.regressions.every((item) => item.passed),
  };
}
