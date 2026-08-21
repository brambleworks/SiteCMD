const PAYLOAD_DIR = "apps/desktop/src-tauri/crates/engine/browser";
const PAYLOAD_MODULE = "apps/desktop/src-tauri/crates/engine/src/browser/payload.rs";
const FACTS_MODULE = "apps/desktop/src-tauri/crates/engine/src/browser/facts.rs";
const PAYLOAD_ASSETS = ["axe.min.js", "cwv_observer.js", "cwv_read.js"];

// axe buckets required to prove both findings and executed absences.
const AXE_BUCKETS = ["violations", "passes", "incomplete", "inapplicable"];

// Only the shared payload module may author the injected script. Anything else
// matching these markers is a second copy of the browser contract.
const AXE_AUTHORING_MARKERS = [/axe\.run\s*\(/, /runOnly/];

const RUST_ROOTS = ["apps/desktop/src-tauri/src", "apps/desktop/src-tauri/crates"];

export function browserPayloadFailures(read, listFiles, exists) {
  const failures = [];

  for (const asset of PAYLOAD_ASSETS) {
    if (!exists(`${PAYLOAD_DIR}/${asset}`)) {
      failures.push(
        `${PAYLOAD_DIR}/${asset} must stay in the engine crate's shared browser directory; every runtime (desktop webview, CLI headless Chrome, hosted runner) injects these same bytes.`,
      );
    }
  }

  const payload = read(PAYLOAD_MODULE);
  const missingBuckets = AXE_BUCKETS.filter((bucket) => !payload.includes(`results.${bucket}`));
  if (missingBuckets.length > 0) {
    failures.push(
      `${PAYLOAD_MODULE} must return rule ids for all four axe buckets (missing: ${missingBuckets.join(", ")}); without them an absent finding cannot be told from a rule that never executed.`,
    );
  }
  if (!/pub const AXE_RUN_TAGS/.test(payload)) {
    failures.push(
      `${PAYLOAD_MODULE} must define AXE_RUN_TAGS: the WCAG tag set decides which rules execute, so it is a comparability fact and cannot live in an adapter.`,
    );
  }
  if (!/pub struct AxeEvidenceCaps/.test(payload)) {
    failures.push(
      `${PAYLOAD_MODULE} must define AxeEvidenceCaps so every runtime bounds violation evidence identically.`,
    );
  }

  const facts = read(FACTS_MODULE);
  for (const bucket of ["passes", "incomplete", "inapplicable"]) {
    if (!new RegExp(`pub ${bucket}: Vec<String>`).test(facts)) {
      failures.push(
        `${FACTS_MODULE}: AxeReport must carry the \`${bucket}\` rule ids; rule-level coverage is what lets a vanished finding count as a fix.`,
      );
    }
  }

  const isRustSource = (file) => file.endsWith(".rs");
  const authors = RUST_ROOTS.flatMap((root) => listFiles(root, isRustSource)).filter((file) => {
    if (file === PAYLOAD_MODULE) return false;
    const source = read(file);
    return AXE_AUTHORING_MARKERS.some((marker) => marker.test(source));
  });
  if (authors.length > 0) {
    failures.push(
      `Only ${PAYLOAD_MODULE} may author the axe run script (axe.run / runOnly). A second copy drifts from the shared payload and breaks browser-tier parity: ${authors.join(", ")}`,
    );
  }

  return failures;
}
