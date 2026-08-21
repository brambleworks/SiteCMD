const SCAN_LABEL_SOURCE = "apps/desktop/src/lib/scan-labels.ts";
const SCAN_LABEL_TEST = "apps/desktop/src/lib/scan-labels.test.ts";
// Rust generates these types: web focuses use ScanType, while Code Scan uses
// ScheduledScanType. lib/types.ts re-exports both.
const GENERATED_TYPES = "apps/desktop/src/generated/ipc-bindings.ts";

export function desktopScanLabelFailures(read, sourceFiles) {
  const failures = [];
  const source = read(SCAN_LABEL_SOURCE);
  const testSource = read(SCAN_LABEL_TEST);
  const generatedSource = read(GENERATED_TYPES);

  if (
    !source.includes("SCAN_LABELS") ||
    !source.includes("WEB_SCAN_SUBTYPE_LABELS") ||
    !source.includes("getScanArtifactLabel")
  ) {
    failures.push(
      `${SCAN_LABEL_SOURCE} must remain the single frontend source for scan family/subtype labels.`,
    );
  }

  for (const scanType of ["health", "security", "accessibility", "polish"]) {
    if (!new RegExp(`export type ScanType =[^;]*"${scanType}"`, "s").test(generatedSource)) {
      failures.push(
        `${GENERATED_TYPES} ScanType must include "${scanType}" so frontend scan labels match the scan engines.`,
      );
    }
  }
  for (const scanType of ["health", "security", "accessibility", "polish", "code"]) {
    if (
      !new RegExp(`export type ScheduledScanType =[^;]*"${scanType}"`, "s").test(generatedSource)
    ) {
      failures.push(
        `${GENERATED_TYPES} ScheduledScanType must include "${scanType}" so scheduled scan labels match the scan engines.`,
      );
    }
  }

  for (const labelCase of [
    '"health"',
    '"security"',
    '"accessibility"',
    '"polish"',
    '"code"',
    '"session"',
  ]) {
    if (!testSource.includes(`getScanArtifactLabel(${labelCase}`)) {
      failures.push(`${SCAN_LABEL_TEST} must pin ${labelCase} scan label behavior.`);
    }
  }

  const hardcodedSubtypeLabel =
    /["'`](Web Scan · (Full|Security|Accessibility|Polish)|Multi-page Web Scan)["'`]/;
  const hardcodedFiles = sourceFiles.filter(
    (file) =>
      file !== SCAN_LABEL_SOURCE &&
      !/\.(test|spec)\.[cm]?[tj]sx?$/.test(file) &&
      hardcodedSubtypeLabel.test(read(file)),
  );
  if (hardcodedFiles.length > 0) {
    failures.push(
      `Scan subtype labels must come from ${SCAN_LABEL_SOURCE}, not hardcoded strings: ${hardcodedFiles.join(", ")}`,
    );
  }

  return failures;
}
