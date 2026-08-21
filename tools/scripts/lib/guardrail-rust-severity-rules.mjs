const VOCAB_SOURCE = "apps/desktop/src-tauri/crates/engine/src/vocab.rs";
const ROOTS = ["apps/desktop/src-tauri/src", "apps/desktop/src-tauri/crates"];
const SEVERITY_COPY_PATTERNS = [
  /Severity::Critical\s*=>\s*0[\s\S]{0,180}Severity::Low\s*=>\s*3/,
  /Some\("critical"\)\s*=>\s*0[\s\S]{0,180}Some\("low"\)\s*=>\s*3/,
  /Severity::Critical\s*=>\s*"critical"[\s\S]{0,180}Severity::Low\s*=>\s*"low"/,
  // String ranks must use the canonical helpers.
  /"critical"\s*=>\s*[0-9]/,
];
const CATEGORY_COPY_PATTERNS = [
  /ScanCategory::Security\s*=>\s*"security"[\s\S]{0,280}ScanCategory::Polish\s*=>\s*"polish"/,
  /ScanCategory::Security\s*=>\s*"Security"[\s\S]{0,280}ScanCategory::Polish\s*=>\s*"Polish"/,
];

export function rustSeverityConsistencyFailures(read, listFiles) {
  const isSource = (file) =>
    file.endsWith(".rs") &&
    !/[/\\]tests[/\\]/.test(file) &&
    !/(_tests?\.rs|^tests\.rs)$/.test(file.split(/[/\\]/).pop());
  const files = ROOTS.flatMap((root) => listFiles(root, isSource));
  const allowed = new Set([VOCAB_SOURCE, "apps/desktop/src-tauri/src/core/code_scan/types.rs"]);
  const duplicateFiles = files.filter(
    (file) => !allowed.has(file) && SEVERITY_COPY_PATTERNS.some((re) => re.test(read(file))),
  );
  // String comparisons indicate a severity field lost its enum type.
  const stringComparisonFiles = files.filter((file) =>
    /\.severity\s*[!=]=\s*"(critical|high|medium|low)"/.test(read(file)),
  );
  const categoryCopyFiles = files.filter(
    (file) => file !== VOCAB_SOURCE && CATEGORY_COPY_PATTERNS.some((re) => re.test(read(file))),
  );
  const failures = [];
  if (duplicateFiles.length > 0) {
    failures.push(
      `Rust issue severity string/rank helpers must use the engine vocab's Severity methods instead of local match copies: ${duplicateFiles.join(", ")}`,
    );
  }
  if (stringComparisonFiles.length > 0) {
    failures.push(
      `Rust issue severity is the typed checks::Severity enum; compare against Severity::* variants, not string literals: ${stringComparisonFiles.join(", ")}`,
    );
  }
  if (categoryCopyFiles.length > 0) {
    failures.push(
      `Rust scan category string/display helpers must use the engine vocab's ScanCategory methods instead of local match copies: ${categoryCopyFiles.join(", ")}`,
    );
  }
  return failures;
}
