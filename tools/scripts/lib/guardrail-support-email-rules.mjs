const EMAIL_RE = /(?:support|hello)@sitecmd\.com/;

// The single sources of truth - the only files allowed to hold the literal.
const CANONICAL = new Set(["apps/desktop/src/lib/support.ts"]);

// Roots that expose an importable SUPPORT_EMAIL constant.
const SOURCE_ROOTS = ["apps/desktop/src"];

function isScannableSource(file) {
  if (!/\.(?:ts|tsx|astro)$/.test(file)) return false;
  if (/\.test\.(?:ts|tsx)$/.test(file)) return false;
  return true;
}

export function supportEmailLiteralFailures(read, exists, listFiles) {
  const failures = [];
  for (const root of SOURCE_ROOTS) {
    if (!exists(root)) continue;
    for (const file of listFiles(root, isScannableSource)) {
      if (CANONICAL.has(file)) continue;
      const lines = read(file).split("\n");
      for (let i = 0; i < lines.length; i += 1) {
        if (EMAIL_RE.test(lines[i])) {
          failures.push(
            `${file}:${i + 1} - inline support email literal; import SUPPORT_EMAIL from the app's lib/support instead. Line: ${lines[i].trim()}`,
          );
        }
      }
    }
  }
  return failures;
}
