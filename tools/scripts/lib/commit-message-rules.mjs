const MAX_SUBJECT_LENGTH = 60;
const MAX_SUBJECT_WORDS = 10;
const MAX_BODY_LENGTH = 400;
const MAX_BODY_LINES = 4;

const CONVENTIONAL_TYPES = new Set([
  "build",
  "chore",
  "ci",
  "docs",
  "feat",
  "fix",
  "perf",
  "refactor",
  "revert",
  "style",
  "test",
]);
const TICKET_PREFIX = /^(?:\[[^\]]+\]|[A-Z][A-Z0-9]+-\d+:?)\s+/;
const NON_IMPERATIVE_OPENING =
  /^(?:Added|Changed|Created|Documented|Fixed|Hardened|Implemented|Improved|Moved|Refactored|Removed|Renamed|Updated|Adding|Changing|Creating|Documenting|Fixing|Hardening|Implementing|Improving|Moving|Refactoring|Removing|Renaming|Updating)\b/;
const VAGUE_SUBJECT =
  /^(?:(?:more|miscellaneous|misc) changes?|updates?|fix(?:es)?|cleanup|wip|work in progress|(?:fix|address|resolve|handle) (?:issues|feedback|findings|problems)|(?:public )?release (?:hardening|readiness)|hardening)$/i;
const TRAILER = /^(?:Co-authored-by|Signed-off-by|Reviewed-by|Refs|Closes|Fixes):\s+\S/i;

function visibleLines(message) {
  const lines = String(message ?? "")
    .replace(/\r\n?/g, "\n")
    .split("\n");
  const scissors = lines.findIndex((line) => /^# -+ >8 -+/.test(line));
  const beforeScissors = scissors === -1 ? lines : lines.slice(0, scissors);
  return beforeScissors.filter((line) => !line.startsWith("#"));
}

function isTrailer(line) {
  return TRAILER.test(line) || /^\[budget-raised:\s+.+\]$/i.test(line);
}

function hasConventionalPrefix(subject) {
  const separator = subject.indexOf(":");
  if (separator < 1 || !/\s/u.test(subject[separator + 1] ?? "")) return false;

  let prefix = subject.slice(0, separator);
  if (prefix.endsWith("!")) prefix = prefix.slice(0, -1);

  const scopeStart = prefix.indexOf("(");
  const type = scopeStart === -1 ? prefix : prefix.slice(0, scopeStart);
  if (!CONVENTIONAL_TYPES.has(type.toLowerCase())) return false;
  if (scopeStart === -1) return true;

  return prefix.endsWith(")") && !prefix.slice(scopeStart + 1, -1).includes(")");
}

export function commitMessageFailures(message, { subjectOnly = false } = {}) {
  const lines = visibleLines(message);
  while (lines.length > 0 && lines.at(-1).trim() === "") lines.pop();
  while (lines.length > 0 && lines[0].trim() === "") lines.shift();

  const subject = lines[0]?.trim() ?? "";
  if (!subject) return ["Write a commit subject."];

  const failures = [];
  const words = subject.split(/\s+/u);

  if (subject.length > MAX_SUBJECT_LENGTH) {
    failures.push(`Keep the subject at ${MAX_SUBJECT_LENGTH} characters or fewer.`);
  }
  if (words.length < 2) {
    failures.push("Name both an action and the thing it changes.");
  } else if (words.length > MAX_SUBJECT_WORDS) {
    failures.push(`Keep the subject at ${MAX_SUBJECT_WORDS} words or fewer.`);
  }
  if (!/^[A-Z]/.test(subject)) {
    failures.push("Start the subject with a capitalized imperative verb.");
  }
  if (hasConventionalPrefix(subject)) {
    failures.push("Remove the Conventional Commit prefix.");
  }
  if (TICKET_PREFIX.test(subject)) {
    failures.push("Move ticket references out of the subject prefix.");
  }
  if (subject.includes(":")) {
    failures.push("Write one plain-English subject without a prefix or colon.");
  }
  if (/[.!?;:]$/.test(subject)) {
    failures.push("Do not end the subject with punctuation.");
  }
  if (NON_IMPERATIVE_OPENING.test(subject)) {
    failures.push("Use the imperative form, such as Add, Fix, Remove, or Update.");
  }
  if (VAGUE_SUBJECT.test(subject)) {
    failures.push("Name the specific behavior or component being changed.");
  }

  if (!subjectOnly) {
    const hasBody = lines.slice(1).some((line) => line.trim() !== "");
    if (hasBody && lines[1]?.trim() !== "") {
      failures.push("Leave a blank line between the subject and body.");
    }

    const bodyLines = lines
      .slice(2)
      .map((line) => line.trim())
      .filter((line) => line !== "" && !isTrailer(line));
    const bodyLength = bodyLines.join("\n").length;
    if (bodyLines.length > MAX_BODY_LINES) {
      failures.push(`Keep the body at ${MAX_BODY_LINES} non-empty lines or fewer.`);
    }
    if (bodyLength > MAX_BODY_LENGTH) {
      failures.push(`Keep the body at ${MAX_BODY_LENGTH} characters or fewer.`);
    }
  }

  return [...new Set(failures)];
}
