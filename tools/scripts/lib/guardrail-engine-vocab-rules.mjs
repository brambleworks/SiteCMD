const VOCAB_SOURCE = "apps/desktop/src-tauri/crates/engine/src/vocab.rs";
const CHECKS_MOD = "apps/desktop/src-tauri/src/checks/mod.rs";

const SEVERITY_HELPER_PINS = [
  "pub fn as_str",
  "pub fn label",
  "pub fn sort_rank",
  "pub fn sort_rank_for_label",
  "severity_string_helpers_are_canonical",
];

const CATEGORY_HELPER_PINS = [
  "impl ScanCategory",
  "pub fn display_label",
  "scan_category_string_helpers_are_canonical",
];

export function engineVocabFailures(read) {
  const vocabSource = read(VOCAB_SOURCE);
  const failures = [];
  const missingSeverity = SEVERITY_HELPER_PINS.filter((pin) => !vocabSource.includes(pin));
  if (missingSeverity.length > 0) {
    failures.push(
      `${VOCAB_SOURCE} must keep the canonical Severity helper surface (missing: ${missingSeverity.join(", ")}); the desktop and hosted scorer both resolve severity strings/ranks through it.`,
    );
  }
  const missingCategory = CATEGORY_HELPER_PINS.filter((pin) => !vocabSource.includes(pin));
  if (missingCategory.length > 0) {
    failures.push(
      `${VOCAB_SOURCE} must keep the canonical ScanCategory helper surface (missing: ${missingCategory.join(", ")}).`,
    );
  }
  const checksMod = read(CHECKS_MOD);
  if (!/pub use sitecmd_engine::\{[^}]*CheckStatus[\s\S]{0,120}Severity[^}]*\}/.test(checksMod)) {
    failures.push(
      `${CHECKS_MOD} must re-export the engine vocabulary (pub use sitecmd_engine::{CheckResult, CheckStatus, ..., Severity}) so crate::checks stays the desktop's single import path.`,
    );
  }
  if (!checksMod.includes("pub use sitecmd_engine::{Check, PageContext};")) {
    failures.push(
      `${CHECKS_MOD} must re-export the engine sync check surface (pub use sitecmd_engine::{Check, PageContext};) so desktop checks implement the ONE portable trait the hosted runner executes.`,
    );
  }
  return failures;
}

// Production checks use the caller's evaluation time so every runner agrees.
const CLOCK_FREE_ROOTS = ["apps/desktop/src-tauri/src/checks", "apps/desktop/src-tauri/crates"];
const AMBIENT_CLOCK = /\b(?:Utc|Local)::now\s*\(|\bSystemTime::now\s*\(/;

export function ambientClockFailures(read, listFiles) {
  const isNonTestSource = (file) =>
    file.endsWith(".rs") &&
    !/[/\\]tests[/\\]/.test(file) &&
    !/(_tests?\.rs|^tests\.rs)$/.test(file.split(/[/\\]/).pop());
  const offenders = CLOCK_FREE_ROOTS.flatMap((root) => listFiles(root, isNonTestSource)).filter(
    (file) => {
      const source = read(file);
      const cut = source.indexOf("#[cfg(test)]");
      const nonTest = cut === -1 ? source : source.slice(0, cut);
      return nonTest
        .split("\n")
        .some((line) => AMBIENT_CLOCK.test(line) && !line.includes("// allow-ambient-clock"));
    },
  );
  if (offenders.length === 0) return [];
  return [
    `Checks and the engine crate must take their time basis from the injected evaluation_time (CheckContext / caller argument), never an ambient clock (Utc::now/Local::now/SystemTime::now; annotate a genuine exception with // allow-ambient-clock): ${offenders.join(", ")}`,
  ];
}
