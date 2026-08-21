import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const LIST_ENTRY = /^\s*[-*]\s+\S/;
const RELEASE_DATE = /^\d{4}-\d{2}-\d{2}$/;

function isReleaseVersion(value) {
  const separator = value.indexOf("-");
  const core = separator === -1 ? value : value.slice(0, separator);
  const prerelease = separator === -1 ? null : value.slice(separator + 1);
  const numeric = (part) =>
    part.length > 0 && [...part].every((character) => character >= "0" && character <= "9");
  const prereleaseCharacter = (character) =>
    (character >= "0" && character <= "9") ||
    (character >= "A" && character <= "Z") ||
    (character >= "a" && character <= "z") ||
    character === "." ||
    character === "-";
  const parts = core.split(".");
  return (
    parts.length === 3 &&
    parts.every(numeric) &&
    (prerelease === null || (prerelease.length > 0 && [...prerelease].every(prereleaseCharacter)))
  );
}

function trimBlankLines(lines) {
  let start = 0;
  let end = lines.length;
  while (start < end && lines[start].trim() === "") start += 1;
  while (end > start && lines[end - 1].trim() === "") end -= 1;
  return lines.slice(start, end);
}

function inspectUnreleasedSection(source) {
  const newline = source.includes("\r\n") ? "\r\n" : "\n";
  const lines = source.split(/\r?\n/);
  const start = lines.findIndex((line) => /^##\s+\[Unreleased\]/i.test(line));
  if (start === -1) {
    return { ok: false, reason: "no `## [Unreleased]` section found" };
  }
  const nextHeading = lines.slice(start + 1).findIndex((line) => /^##\s/.test(line));
  const end = nextHeading === -1 ? lines.length : start + 1 + nextHeading;
  const content = trimBlankLines(lines.slice(start + 1, end));
  if (!content.some((line) => LIST_ENTRY.test(line))) {
    return {
      ok: false,
      reason: "the Unreleased section has no list entries, only prose or nothing",
    };
  }
  return {
    ok: true,
    content,
    end,
    lines,
    newline,
    notes: content.join("\n"),
    start,
  };
}

function changelogNotReady(reason) {
  return new Error(
    `CHANGELOG.md is not ready to release: ${reason}. ` +
      "Write the release notes as list entries under `## [Unreleased]` first.",
  );
}

export function evaluateChangelogNotes(source) {
  const inspected = inspectUnreleasedSection(source);
  return inspected.ok
    ? { ok: true, notes: inspected.notes }
    : { ok: false, reason: inspected.reason };
}

export function formatLocalReleaseDate(date) {
  if (!(date instanceof Date) || Number.isNaN(date.getTime())) {
    throw new Error("cannot format an invalid release date");
  }
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function extractReleaseNotes({ source, version }) {
  if (!isReleaseVersion(version)) {
    throw new Error(`invalid changelog release version: ${version}`);
  }
  const lines = source.split(/\r?\n/);
  const matchingHeadings = lines
    .map((line, index) => ({ index, version: /^##\s+\[([^\]]+)\]/.exec(line)?.[1] }))
    .filter((heading) => heading.version === version);
  if (matchingHeadings.length !== 1) {
    throw new Error(
      matchingHeadings.length === 0
        ? `CHANGELOG.md has no release heading for ${version}`
        : `CHANGELOG.md has duplicate release headings for ${version}`,
    );
  }

  const start = matchingHeadings[0].index;
  const nextHeading = lines.slice(start + 1).findIndex((line) => /^##\s/.test(line));
  const end = nextHeading === -1 ? lines.length : start + 1 + nextHeading;
  const content = trimBlankLines(lines.slice(start + 1, end));
  if (!content.some((line) => LIST_ENTRY.test(line))) {
    throw new Error(`CHANGELOG.md release ${version} has no list entries`);
  }
  return content.join("\n");
}

export function prepareChangelogRelease({ source, version, releaseDate }) {
  if (!isReleaseVersion(version)) {
    throw new Error(`invalid changelog release version: ${version}`);
  }
  if (!RELEASE_DATE.test(releaseDate)) {
    throw new Error(`invalid changelog release date: ${releaseDate}`);
  }

  const inspected = inspectUnreleasedSection(source);
  if (!inspected.ok) throw changelogNotReady(inspected.reason);

  const duplicate = inspected.lines.some(
    (line) => /^##\s+\[([^\]]+)\]/.exec(line)?.[1] === version,
  );
  if (duplicate) {
    throw new Error(`CHANGELOG.md already contains a release heading for ${version}`);
  }

  const output = [
    ...inspected.lines.slice(0, inspected.start + 1),
    "",
    `## [${version}] - ${releaseDate}`,
    "",
    ...inspected.content,
  ];
  const tail = inspected.lines.slice(inspected.end);
  if (tail.length > 0) output.push("", ...tail);

  let preparedSource = output.join(inspected.newline);
  if (/\r?\n$/.test(source) && !preparedSource.endsWith(inspected.newline)) {
    preparedSource += inspected.newline;
  }
  return { notes: inspected.notes, source: preparedSource };
}

export function assertChangelogReady({ changelogPath }) {
  let source;
  try {
    source = fs.readFileSync(changelogPath, "utf8");
  } catch {
    throw new Error(`cannot read ${changelogPath}; a release ships its changelog`);
  }
  const verdict = evaluateChangelogNotes(source);
  if (!verdict.ok) throw changelogNotReady(verdict.reason);
  return verdict;
}

const modulePath = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  const [command, version] = process.argv.slice(2);
  if (command !== "--release-notes" || !version) {
    console.error("usage: node check-changelog-notes.mjs --release-notes X.Y.Z");
    process.exitCode = 1;
  } else {
    try {
      const source = fs.readFileSync(path.resolve("CHANGELOG.md"), "utf8");
      process.stdout.write(`${extractReleaseNotes({ source, version })}\n`);
    } catch (error) {
      console.error(`release-notes: ${error.message}`);
      process.exitCode = 1;
    }
  }
}
