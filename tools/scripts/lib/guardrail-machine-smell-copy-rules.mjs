const ALLOW_MARKER = "allow-machine-smell";

const BANNED = [
  {
    re: /\bunlock(?:s|ed|ing)?\b/i,
    hint: 'the upgrade-verb tell "unlock"; use a concrete verb (Get / See / Show / Add / Enable / Activate)',
    isExempt: () => false,
  },
  {
    re: /\bqueue[ds]?\b/i,
    hint: 'the AI-tell "queue" in copy; use "list" (or "unsent" / "pending" / "started" for status)',
    isExempt: (file) => file.includes("code-fix-guides/"),
  },
];

const SKIP_FILE_RE =
  /(?:\.test\.|\.spec\.|\/generated\/|\/__tests__\/|FixQueue|AttentionOverlay|PreDeployBanner)/;

const FRONTEND_DIRS = ["apps/desktop/src", "apps/mcp-server/src"];
const FRONTEND_EXT_RE = /\.(?:tsx|ts|astro)$/;

// Backend files that emit visible notification or alert copy.
const RUST_COPY_FILES = ["apps/desktop/src-tauri/src/core/native_alerts.rs"];

export function visibleCopySpans(line) {
  const spans = [];
  // Keep literal matching at star-height one for the safe-regex audit.
  const quoteRe = /"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g;
  let match;
  while ((match = quoteRe.exec(line)) !== null) {
    const raw = match[1] ?? match[2] ?? match[3] ?? "";
    spans.push(raw.replace(/\$\{[^}]*\}/g, " "));
  }
  const jsxRe = />([^<>{}]+)</g;
  while ((match = jsxRe.exec(line)) !== null) {
    spans.push(match[1]);
  }
  return spans.filter((span) => {
    const tokens = span.trim().split(/\s+/);
    if (tokens.length < 2) return false;
    // Ignore class-token lists that contain no prose.
    if (tokens.every((token) => /[-:/]/.test(token))) return false;
    return true;
  });
}

function scanFile(read, file, failures) {
  const lines = read(file).split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.includes(ALLOW_MARKER)) continue;
    const spans = visibleCopySpans(line);
    if (spans.length === 0) continue;
    for (const rule of BANNED) {
      if (rule.isExempt(file)) continue;
      if (spans.some((span) => rule.re.test(span))) {
        failures.push(`${file}:${i + 1} - ${rule.hint}. Line: ${line.trim()}`);
      }
    }
  }
}

export function queueUnlockCopyFailures(read, exists, listFiles) {
  const failures = [];
  const scanned = new Set();
  for (const dir of FRONTEND_DIRS) {
    if (!exists(dir)) continue;
    for (const file of listFiles(dir, (f) => FRONTEND_EXT_RE.test(f) && !SKIP_FILE_RE.test(f))) {
      scanned.add(file);
    }
  }
  for (const file of RUST_COPY_FILES) {
    if (exists(file)) scanned.add(file);
  }
  for (const file of scanned) {
    scanFile(read, file, failures);
  }
  return failures;
}
