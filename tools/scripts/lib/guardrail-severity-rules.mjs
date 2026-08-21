export function desktopSeverityConsistencyFailures(read, sourceFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const severitySource = read("apps/desktop/src/lib/severity.ts");
  const severityTests = read("apps/desktop/src/lib/severity.test.ts");
  const domainSpecificSeverityLabelFiles = new Set([
    "apps/desktop/src/components/alerts/alert-display.ts",
    "apps/desktop/src/components/dashboard/search-console-page-model.ts",
    "apps/desktop/src/lib/score.ts",
  ]);
  const desktopFiles = sourceFiles.filter(
    (file) =>
      file.startsWith("apps/desktop/src/") &&
      /\.(ts|tsx)$/.test(file) &&
      !file.includes(".test.") &&
      file !== "apps/desktop/src/lib/severity.ts",
  );
  const generalFiles = desktopFiles.filter((file) => !domainSpecificSeverityLabelFiles.has(file));
  const inlineSeverityCountFiles = desktopFiles.filter((file) => {
    const source = read(file);
    return (
      /critical:\s*0,\s*high:\s*0,\s*medium:\s*0,\s*low:\s*0/s.test(source) ||
      /interface\s+\w*(?:Severity|Issue)Counts\s*\{[^}]*critical:\s*number;[^}]*high:\s*number;[^}]*medium:\s*number;[^}]*low:\s*number;/s.test(
        source,
      ) ||
      source.includes(
        "severityCounts.critical + severityCounts.high + severityCounts.medium + severityCounts.low",
      )
    );
  });
  const inlineSeverityLabelFiles = generalFiles.filter((file) => {
    const source = read(file);
    return (
      source.includes("severity.charAt(0).toUpperCase() + severity.slice(1)") ||
      /return "Critical";[\s\S]*return "High";[\s\S]*return "Medium";[\s\S]*return "Low";/.test(
        source,
      ) ||
      /function\s+formatIssueSeverity[\s\S]{0,180}charAt\(0\)\.toUpperCase\(\)\s*\+\s*severity\.slice\(1\)/.test(
        source,
      )
    );
  });
  const inlineSeverityStylingFiles = generalFiles.filter(
    (file) =>
      /\bcritical\s*:[\s\S]{0,120}?\btext-[\s\S]{0,220}?\bhigh\s*:[\s\S]{0,220}?\bmedium\s*:/.test(
        read(file),
      ) ||
      /\bseverity\s*===\s*["'](?:critical|high|medium|low)["'][\s\S]{0,120}?["'`][^"'`\n]*\b(?:text|bg|border)-/.test(
        read(file),
      ),
  );

  check(
    severitySource.includes("createSeverityCounts") &&
      severitySource.includes("addSeverityCounts") &&
      severitySource.includes("severityCountTotal") &&
      severitySource.includes("formatSeverityLabel") &&
      severitySource.includes("formatSeverityToneClass") &&
      severitySource.includes("severityCssVar") &&
      severitySource.includes("isSeverity") &&
      severityTests.includes("creates, adds, and totals severity count records") &&
      severityTests.includes("formatSeverityLabel") &&
      inlineSeverityCountFiles.length === 0,
    `Desktop issue severity count records/totals must use lib/severity.ts helpers instead of local critical/high/medium/low object or sum copies: ${inlineSeverityCountFiles.join(", ")}`,
  );
  check(
    inlineSeverityLabelFiles.length === 0,
    `Desktop generic issue severity labels/colors must use lib/severity.ts helpers instead of local Critical/High/Medium/Low label maps: ${inlineSeverityLabelFiles.join(", ")}`,
  );
  check(
    desktopFiles.length >= 300 && inlineSeverityStylingFiles.length === 0,
    `Desktop issue severity styling must route through lib/severity.ts severityToneClass instead of inline severity className maps or severity === "critical" className branches (fails closed under 300 scanned files): ${inlineSeverityStylingFiles.join(", ")}`,
  );

  return failures;
}
