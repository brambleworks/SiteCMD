const PRODUCT_DOCS = "docs/product";
const DOCS_INDEX = "docs/README.md";
const CONTRIBUTING = "CONTRIBUTING.md";
const CONNECTED_SPECS = "docs/engineering/connected-service";
const LOCALHOST_FIXTURES = "apps/desktop/src-tauri/src/core/localhost.rs";

// Product-intent phrasing belongs in docs/qa, where reviewers read it.
const INTENT_PHRASES =
  /SiteCMD should|treat that as product feedback|should make (?:that|the) [a-z-]+ (?:step )?obvious/i;
// Plain-text names of private records are allowed; deferring a rule to one is not.
const PRIVATE_RECORD_DEFERRAL = /commercial (?:terms )?spec|RFC's economics section/i;
// desktop-repo-public-face plan Task 8 rewrites this walkthrough; exclude it here until then.
const WALKTHROUGH_PENDING_TASK_8 = `${PRODUCT_DOCS}/get-value-in-5-minutes.md`;

export function publicFaceFailures(read, exists, listFiles) {
  const failures = [];

  for (const file of listFiles(
    PRODUCT_DOCS,
    (path) => path.endsWith(".md") && path !== WALKTHROUGH_PENDING_TASK_8,
  )) {
    const hit = INTENT_PHRASES.exec(read(file));
    if (hit) {
      failures.push(
        `${file} reads as product intent ("${hit[0]}"); a walkthrough describes what the app does, and intent lines belong in docs/qa/manual-testing-runbook.md.`,
      );
    }
  }

  const docsIndex = read(DOCS_INDEX);
  if (!/publication-checklist\.md\)[^\n]*historical/i.test(docsIndex)) {
    failures.push(
      `${DOCS_INDEX} must mark operations/publication-checklist.md as historical on the line that links it; the cutover ran on 2026-08-21 and the checklist is no longer an entry point.`,
    );
  }
  if (
    !docsIndex.includes("connected-service") ||
    !read(CONTRIBUTING).includes("docs/engineering/connected-service/")
  ) {
    failures.push(
      `${DOCS_INDEX} and ${CONTRIBUTING} must say the connected-service implementation specifications are public in docs/engineering/connected-service/; the old wording called them private while the files sat beside it.`,
    );
  }

  for (const file of listFiles(CONNECTED_SPECS, (path) => path.endsWith("-spec.md"))) {
    const hit = PRIVATE_RECORD_DEFERRAL.exec(read(file));
    if (hit) {
      failures.push(
        `${file} defers to a private record ("${hit[0]}"); state the rule in place, or say the value is set outside this specification.`,
      );
    }
  }

  if (exists(LOCALHOST_FIXTURES) && /https?:\/\/[a-z0-9.-]+\.ai\b/.test(read(LOCALHOST_FIXTURES))) {
    failures.push(
      `${LOCALHOST_FIXTURES} names a real third-party domain as a test fixture; use a reserved example.com host.`,
    );
  }

  return failures;
}
