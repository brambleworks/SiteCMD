const POLISH_RUST_FILES = [
  "apps/desktop/src-tauri/src/checks/polish/ai_aesthetic.rs",
  "apps/desktop/src-tauri/src/checks/polish/copy_content.rs",
  "apps/desktop/src-tauri/src/checks/polish/css_architecture.rs",
  "apps/desktop/src-tauri/src/checks/polish/framework_defaults.rs",
  "apps/desktop/src-tauri/src/checks/polish/html_quality.rs",
  "apps/desktop/src-tauri/src/checks/polish/meta_infra.rs",
  "apps/desktop/src-tauri/src/checks/polish/titles.rs",
];

// Case-insensitive bans applied only to user-visible string lines.
const BANNED_PATTERNS = [
  { re: /\boverused\b/i, reason: "judgmental - say 'heavy usage' or '<count> detected'" },
  { re: /\bcookie-?cutter\b/i, reason: "judgmental - say 'default scaffold markers'" },
  {
    re: /generic\s+ai-written/i,
    reason: "accusatory - say 'high marketing buzzword density'",
  },
  {
    re: /\bstill on\b/i,
    reason: "judgmental - say 'hosting on <subdomain>' factually",
  },
  {
    re: /\bvibe code\b/i,
    reason: "internal jargon; not for user-visible strings",
  },
  {
    re: /\bvibed\b/i,
    reason: "internal jargon; not for user-visible strings",
  },
  {
    re: /\bChatGPT\b/i,
    reason: "naming a competitor's product in scanner output is unshippable",
  },
  {
    re: /\bsmells like a prompt\b/i,
    reason: "accusatory verdict label",
  },
  {
    re: /\byou both know it\b/i,
    reason: "accusatory second-person phrasing",
  },
  {
    re: /\bhandcrafted\b/i,
    reason: "implies intent the scanner cannot verify",
  },
  {
    re: /AI fingerprint/i,
    reason: "implies intent the scanner cannot verify",
  },
  {
    re: /\bsuspicious\b/i,
    reason: "accusatory - describe the signal count, not a verdict",
  },
];

// Test modules contain the banned phrases as detection needles.
function userVisibleStringLines(content) {
  const lines = content.split("\n");
  const out = [];
  let inTestModule = false;
  let testBraceDepth = 0;
  let pendingTestModule = false;
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const trimmed = line.trimStart();
    if (/#\[cfg\(test\)\]/.test(line)) {
      pendingTestModule = true;
    } else if (pendingTestModule && /\bmod\s+\w+\s*\{/.test(line)) {
      inTestModule = true;
      testBraceDepth = 1;
      pendingTestModule = false;
      continue;
    }
    if (inTestModule) {
      for (const ch of line) {
        if (ch === "{") testBraceDepth += 1;
        else if (ch === "}") testBraceDepth -= 1;
      }
      if (testBraceDepth <= 0) {
        inTestModule = false;
        testBraceDepth = 0;
      }
      continue;
    }
    if (trimmed.startsWith("//")) continue;
    if (!/"[^"\n]*"/.test(line)) continue;
    out.push({ line, index: i + 1 });
  }
  return out;
}

// One runner entry covers both user-facing copy checks.
import { accessibilityNamingFailures } from "./guardrail-accessibility-naming-rules.mjs";

export function polishCopySafetyFailures(read, exists, listFiles) {
  const failures = [...accessibilityNamingFailures(read, exists, listFiles)];
  for (const relativePath of POLISH_RUST_FILES) {
    if (!exists(relativePath)) {
      failures.push(
        `${relativePath} is missing - polish copy guardrail expects all polish source files to exist.`,
      );
      continue;
    }
    const content = read(relativePath);
    const lines = userVisibleStringLines(content);
    for (const { line, index } of lines) {
      for (const { re, reason } of BANNED_PATTERNS) {
        if (re.test(line)) {
          failures.push(
            `${relativePath}:${index} - banned polish copy: ${re}. ${reason}. Line: ${line.trim()}`,
          );
        }
      }
    }
  }
  return failures;
}
