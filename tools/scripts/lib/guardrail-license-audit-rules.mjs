import { LIFECYCLE_ACTIVATION } from "./guardrail-license-sources.mjs";
import { stripComments, stripNonCode } from "./guardrail-source-text.mjs";

const COMMAND = "pub(super) async fn activate_license_with_ports";
const AFTER = "struct DesktopActivationPorts";

// Whole-word patterns that identify raw license-key data in audit arguments.
const RAW_KEY_PATTERNS = [/\bkey\b/, /license_key/, /catalog_key/];

// Audit arguments may mention only these known-safe names.
const AUDIT_ARGUMENT_NAMES = new Set([
  "audit_detail",
  "clone",
  "json",
  "key_fingerprint",
  "serde_json",
  "tier",
  "to_string",
]);

const EXPECTED_BINDINGS = [
  ["key_fingerprint", "let key_fingerprint = license_key_fingerprint(&key);"],
  ["audit_detail", "let audit_detail = license_activation_audit_detail(&key_fingerprint);"],
];

// Count whole-word identifier occurrences.
function occurrences(text, name) {
  return (text.match(new RegExp(`\\b${name}\\b`, "g")) ?? []).length;
}

export function activationAuditConstructionFailures(read) {
  const path = LIFECYCLE_ACTIVATION;
  // Keep literals for record inspection and code only for identifier counting.
  const raw = read(path);
  const commented = stripComments(raw, path);
  const code = stripNonCode(raw, path);
  const start = commented.indexOf(COMMAND);
  const end = start === -1 ? -1 : commented.indexOf(AFTER, start);
  if (start === -1 || end === -1) {
    return [
      `${path} must define "${COMMAND}" ahead of "${AFTER}"; the audit-construction rule cannot find its bounds and would report success over nothing`,
    ];
  }
  const failures = [];
  if (!raw.includes('crate::audit_log::record("license.activate", detail, outcome);')) {
    failures.push(`${path} must route the activation port to license.activate audit rows`);
  }
  // Scope binding counts to the command, excluding helper parameters.
  const details = auditDetails(commented.slice(start, end), code.slice(start, end));
  if (details.length === 0) {
    failures.push(
      `${path} must record license.activate audit rows; a rule that finds none of them is checking nothing`,
    );
  }
  for (const detail of details) {
    if (!/audit_detail|key_fingerprint/.test(detail.literal)) {
      failures.push(
        `${path}: a license.activate audit row is built from something other than audit_detail or key_fingerprint (${detail.literal.trim()}). The audit log must carry the fingerprint, never the license key`,
      );
      continue;
    }
    // Inspect every remaining argument and allow only known-safe code identifiers.
    const unexpected = [
      ...new Set(detail.tailCode.match(/\b[A-Za-z_][A-Za-z0-9_]*\b/g) ?? []),
    ].filter((name) => !AUDIT_ARGUMENT_NAMES.has(name));
    if (unexpected.length > 0) {
      failures.push(
        `${path}: a license.activate audit row mentions ${unexpected.map((name) => `\`${name}\``).join(", ")}, which the row is not allowed to carry. Add the name to AUDIT_ARGUMENT_NAMES only after checking it cannot reconstruct the license key - renaming the key into a fresh local is how a denylist of its spellings gets walked around`,
      );
    }
    // Report raw-key spellings explicitly in addition to unfamiliar names.
    const leak = RAW_KEY_PATTERNS.find((pattern) => pattern.test(detail.tail));
    if (leak) {
      failures.push(
        `${path}: a license.activate audit row matches ${leak} beside the fingerprint (${detail.tail.trim()}). The audit log must carry the fingerprint and NOTHING that reconstructs the key`,
      );
    }
  }
  const body = code.slice(start, end);
  for (const [binding, expected] of EXPECTED_BINDINGS) {
    // Occurrence budgets catch shadowing through every Rust binding form.
    const allowed =
      EXPECTED_BINDINGS.reduce((total, [, line]) => total + occurrences(line, binding), 0) +
      details.reduce((total, detail) => total + occurrences(detail.code, binding), 0);
    const found = occurrences(body, binding);
    if (found !== allowed || !body.includes(expected)) {
      failures.push(
        `${path}: activate_license may mention ${binding} ${allowed} times - once where \`${expected}\` binds it, and once per license.activate audit detail that reads it - but mentions it ${found} times. Any other mention binds it again, and the shadow feeds every audit row built afterwards while the safe line stays present`,
      );
    }
  }
  return failures;
}

// Parse every activation audit call across line breaks and nested brackets.
function auditDetails(literalView, codeView) {
  const details = [];
  for (const match of codeView.matchAll(/ports\.audit\(/g)) {
    const open = match.index + match[0].length - 1;
    const close = matchingClose(codeView, open);
    if (close === -1) continue;
    const args = splitArguments(codeView, open + 1, close);
    if (args.length < 2) continue;
    details.push({
      code: codeView.slice(...args[0]),
      literal: literalView.slice(...args[0]),
      tail: literalView.slice(args[0][0], close),
      tailCode: codeView.slice(args[0][0], close),
    });
  }
  return details;
}

// Return spans for top-level call arguments.
function splitArguments(text, from, to) {
  const spans = [];
  let depth = 0;
  let argFrom = from;
  for (let i = from; i < to; i += 1) {
    const ch = text[i];
    if (ch === "(" || ch === "[" || ch === "{") depth += 1;
    else if (ch === ")" || ch === "]" || ch === "}") depth -= 1;
    else if (ch === "," && depth === 0) {
      spans.push([argFrom, i]);
      argFrom = i + 1;
    }
  }
  spans.push([argFrom, to]);
  return spans;
}

/** Return the matching close parenthesis index, or `-1`. */
export function matchingClose(text, open) {
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    if (text[i] === "(") depth += 1;
    else if (text[i] === ")") {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}
