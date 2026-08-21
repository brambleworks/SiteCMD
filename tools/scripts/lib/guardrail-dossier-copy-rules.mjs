const MARKER = "allow-verify-fallback";

const SCANNED_DIRS = ["apps/desktop/src/components"];

// Match a verify hint coalesced to a string, including multiline expressions.
const BANNED_FALLBACK = /verify_?[Hh]int\s*(?:\?\?|\|\|)\s*[`"']/;

// Capture each rendered verification callout.
const CALLOUT_BLOCK = /<DossierVerifyCallout\b[^>]*>([\s\S]*?)<\/DossierVerifyCallout>/g;

// Callout content must reference the per-issue hint.
const REFERENCES_HINT = /verify_?[Hh]int/;

// Convert a match offset to a 1-indexed line.
function lineAt(content, index) {
  let line = 1;
  for (let i = 0; i < index && i < content.length; i += 1) {
    if (content[i] === "\n") line += 1;
  }
  return line;
}

export function dossierVerifyCopyFailures(read, exists, listFiles) {
  const failures = [];
  for (const dir of SCANNED_DIRS) {
    if (!exists(dir)) continue;
    for (const file of listFiles(dir, (f) => f.endsWith(".tsx") && !f.includes(".test."))) {
      const content = read(file);
      const lines = content.split("\n");

      const fallback = BANNED_FALLBACK.exec(content);
      if (fallback && !lines[lineAt(content, fallback.index) - 1]?.includes(MARKER)) {
        failures.push(
          `${file}:${lineAt(content, fallback.index)} - a per-issue verify hint must not fall back to a hardcoded string; render nothing when the scanner set no hint.`,
        );
      }

      for (const block of content.matchAll(CALLOUT_BLOCK)) {
        if (REFERENCES_HINT.test(block[1])) continue;
        const openLine = lines[lineAt(content, block.index) - 1] ?? "";
        if (block[0].includes(MARKER) || openLine.includes(MARKER)) continue;
        failures.push(
          `${file}:${lineAt(content, block.index)} - <DossierVerifyCallout> must carry the check-specific verify hint (issue.verifyHint), not generic prose; render nothing when there is no hint.`,
        );
      }
    }
  }
  return failures;
}
