import {
  LIFECYCLE_ACTIVATION,
  LIFECYCLE_ROOT,
  LIFECYCLE_SOURCES,
} from "./guardrail-license-sources.mjs";
import {
  activationAuditConstructionFailures,
  matchingClose,
} from "./guardrail-license-audit-rules.mjs";
import { stripComments, stripNonCode } from "./guardrail-source-text.mjs";

const COMMANDS_DIR = "apps/desktop/src-tauri/src/licensing/commands";
const COMMAND = "pub(super) async fn activate_license_with_ports";
const AFTER = "struct DesktopActivationPorts";
const TYPED_REFUSALS = [
  "activation_error(",
  "activation_error_from_raw(",
  "provider_refusal_error(",
];
const TYPED_PROPAGATORS = ["map_err("];

export function licenseSurfaceFailures(read, listFiles) {
  return [
    ...lifecycleCoverageFailures(read, listFiles),
    ...activationAuditConstructionFailures(read),
    ...typedRefusalFailures(read),
  ];
}

function lifecycleCoverageFailures(read, listFiles) {
  const root = stripComments(read(LIFECYCLE_ROOT), LIFECYCLE_ROOT);
  const declared = [...root.matchAll(/#\[path\s*=\s*"([^"]+\.rs)"\]/g)]
    .map((match) => match[1])
    .filter((module) => !module.endsWith("_tests.rs"))
    .map((module) => `${COMMANDS_DIR}/${module}`);
  const failures = [];
  if (declared.length === 0) {
    return [
      `${LIFECYCLE_ROOT} declares no #[path] lifecycle submodules; the coverage rule found nothing to check, which is itself the failure`,
    ];
  }
  // Include the root and on-disk modules that path declarations cannot discover.
  const present = listFiles(COMMANDS_DIR, (file) => file.endsWith(".rs")).filter((file) => {
    const module = file.slice(file.lastIndexOf("/") + 1);
    return module !== "mod.rs" && !module.endsWith("_tests.rs");
  });
  const onDisk = [...new Set([LIFECYCLE_ROOT, ...declared, ...present])];
  for (const file of onDisk) {
    if (!LIFECYCLE_SOURCES.includes(file)) {
      failures.push(
        `${file} exists but is not in LIFECYCLE_SOURCES (tools/scripts/lib/guardrail-license-sources.mjs). Every rule that reads the lifecycle as one string would search a text that cannot contain it, and the negative ones would report its absence as compliance`,
      );
    }
  }
  return failures;
}

function typedRefusalFailures(read) {
  const path = LIFECYCLE_ACTIVATION;
  // Only executable `return Err` expressions count.
  const source = stripNonCode(read(path), path);
  const start = source.indexOf(COMMAND);
  const end = start === -1 ? -1 : source.indexOf(AFTER, start);
  if (start === -1 || end === -1) {
    return [
      `${path} must define "${COMMAND}" ahead of "${AFTER}"; a rule that cannot find its bounds reports success over nothing at all`,
    ];
  }
  const body = source.slice(start, end);
  const failures = [];
  let returns = 0;
  for (const match of body.matchAll(/return Err\(/g)) {
    returns += 1;
    const open = match.index + match[0].length - 1;
    const close = matchingClose(body, open);
    const inner = body.slice(open + 1, close === -1 ? body.length : close);
    const tail = inner.trimStart();
    if (!TYPED_REFUSALS.some((typed) => tail.startsWith(typed))) {
      failures.push(
        `${path}: activate_license returns Err(${tail.slice(0, 40).trim()}...) without a typed constructor. Every refusal crossing the Tauri boundary must be built by ${TYPED_REFUSALS.join(", ")} or a raw LemonSqueezy body reaches the modal`,
      );
      continue;
    }
    if (inner.includes("+")) {
      failures.push(
        `${path}: activate_license returns Err(${tail.slice(0, 60).trim()}...), which starts with a typed constructor and then concatenates onto it. The payload is JSON the frontend parses; anything appended is a raw provider body riding along inside it`,
      );
    }
  }
  if (returns === 0) {
    failures.push(
      `${path}: activate_license has no \`return Err(\` at all, so the typed-refusal rule is checking nothing`,
    );
  }
  failures.push(...propagationFailures(path, body));
  return failures;
}

function propagationFailures(path, body) {
  const failures = [];
  let previous = -1;
  for (const match of body.matchAll(/\?/g)) {
    if (withinClosure(body, match.index)) continue;
    const receiver = body.slice(
      Math.max(statementStart(body, match.index), previous + 1),
      match.index,
    );
    previous = match.index;
    if (
      TYPED_PROPAGATORS.some(
        (typed) => containsAtTopLevel(receiver, typed) && !mapsToItself(receiver, typed),
      )
    ) {
      continue;
    }
    failures.push(
      `${path}: activate_license propagates \`${receiver.trim().slice(0, 60)}?\` without typing its error. Every \`?\` here must go through map_err into a typed refusal, or a raw String crosses the Tauri boundary and the frontend reads it as "the command never ran"`,
    );
  }
  return failures;
}

function containsAtTopLevel(text, needle) {
  let depth = 0;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (ch === ")" || ch === "]" || ch === "}") depth -= 1;
    // Test before counting the opening parenthesis included in the needle.
    else if (depth === 0 && text.startsWith(needle, i)) return true;
    if (ch === "(" || ch === "[" || ch === "{") depth += 1;
    // Negative depth means this receiver began inside an outer expression.
    if (depth < 0) return false;
  }
  return false;
}

function mapsToItself(text, needle) {
  const at = text.indexOf(needle);
  if (at === -1) return false;
  const open = at + needle.length - 1;
  const close = matchingClose(text, open);
  if (close === -1) return false;
  const argument = text.slice(open + 1, close).trim();
  const closure = /^\|\s*([A-Za-z_][A-Za-z0-9_]*)\s*\|\s*(.+)$/s.exec(argument);
  if (closure) return closure[2].trim() === closure[1];
  // A bare path is a mapper too, and `identity` is the one that types nothing.
  return /(^|::)identity$/.test(argument);
}

// A `?` inside a closure propagates from the closure, not the command.
function withinClosure(text, index) {
  let depth = 0;
  for (let i = index - 1; i >= 0; i -= 1) {
    const ch = text[i];
    if (ch === "}") depth += 1;
    else if (ch === "{") {
      if (depth > 0) depth -= 1;
      else if (text.slice(0, i).trimEnd().endsWith("|")) return true;
    }
  }
  return false;
}

// Find the statement start while skipping nested brackets.
function statementStart(text, index) {
  let depth = 0;
  for (let i = index - 1; i >= 0; i -= 1) {
    const ch = text[i];
    if (ch === ")" || ch === "]" || ch === "}") depth += 1;
    else if (ch === "(" || ch === "[" || ch === "{") {
      if (depth === 0) return i + 1;
      depth -= 1;
    } else if (ch === ";" && depth === 0) return i + 1;
  }
  return 0;
}
