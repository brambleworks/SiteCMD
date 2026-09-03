import fs from "node:fs";
import path from "node:path";

const ADVISORY_PATTERN =
  /\b(?:GHSA-[a-z0-9]{4}-[a-z0-9]{4}-[a-z0-9]{4}|CVE-\d{4}-\d{4,}|RUSTSEC-\d{4}-\d{4})\b/i;
const NON_SECURITY_MARKER = "non-security:";
const PINNED_EXACT_MARKER = "pinned-exact:";
const REVIEW_PATTERN = /^\s*#\s*reviewed:\s*(\d{4}-\d{2}-\d{2})\s*$/;
const REVIEW_WARN_DAYS = 180;

/** Parse overrides with their contiguous justification comments. */
export function parseOverrideEntries(source) {
  const lines = source.split("\n");
  const start = lines.findIndex((line) => /^overrides:\s*$/.test(line));
  if (start === -1) return [];

  const entries = [];
  let comments = [];
  for (let i = start + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim() === "") {
      comments = [];
      continue;
    }
    // The block ends at the first line that is not indented.
    if (!/^\s/.test(line)) break;
    if (/^\s*#/.test(line)) {
      comments.push(line);
      continue;
    }
    const match = /^\s+(?:"([^"]+)"|([^:\s]+)):\s*"([^"]+)"\s*$/.exec(line);
    if (match) {
      entries.push({
        name: match[1] ?? match[2],
        range: match[3],
        comments: [...comments],
        line: i + 1,
      });
    }
    comments = [];
  }
  return entries;
}

function parseVersion(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)/.exec(String(value).trim());
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
}

function compareVersions(a, b) {
  for (let i = 0; i < 3; i += 1) {
    if (a[i] !== b[i]) return a[i] - b[i];
  }
  return 0;
}

/** Lowest version a range can match, or null when it cannot be determined. */
export function rangeFloor(range) {
  const first = String(range)
    .split("||")[0]
    .trim()
    .replace(/^[\^~]|^>=?|^=|^v/g, "")
    .trim();
  return parseVersion(first);
}

/**
 * @returns {{version: number[], inclusive: boolean} | null | undefined}
 * `null` is unbounded; `undefined` is an unsupported comparator shape.
 */
function comparatorCeiling(comparator) {
  const text = comparator.trim();
  if (text === "" || text === "*" || text === "x" || text === "latest") return null;

  const upper = /<\s*=?\s*(\d+\.\d+\.\d+)/.exec(text);
  if (upper) {
    return { version: parseVersion(upper[1]), inclusive: text.includes("<=") };
  }
  if (/^>=?/.test(text)) return null;

  const caret = /^\^\s*(\d+)\.(\d+)\.(\d+)/.exec(text);
  if (caret) {
    const [, major, minor, patch] = caret.map(Number);
    if (major > 0) return { version: [major + 1, 0, 0], inclusive: false };
    if (minor > 0) return { version: [0, minor + 1, 0], inclusive: false };
    return { version: [0, 0, patch + 1], inclusive: false };
  }

  const tilde = /^~\s*(\d+)\.(\d+)\.(\d+)/.exec(text);
  if (tilde) {
    const [, major, minor] = tilde.map(Number);
    return { version: [major, minor + 1, 0], inclusive: false };
  }

  const exact = /^=?\s*(\d+\.\d+\.\d+)$/.exec(text);
  if (exact) return { version: parseVersion(exact[1]), inclusive: true };

  return undefined;
}

/** Whether `range` caps a dependency below `floor`. */
export function rangeCapsBelow(range, floor) {
  const parts = String(range).split("||");
  let sawUnknown = false;
  for (const part of parts) {
    const ceiling = comparatorCeiling(part);
    if (ceiling === undefined) {
      sawUnknown = true;
      continue;
    }
    // Any union member that reaches the floor means the range is not a cap.
    if (ceiling === null) return false;
    const delta = compareVersions(ceiling.version, floor);
    const reaches = ceiling.inclusive ? delta >= 0 : delta > 0;
    if (reaches) return false;
  }
  // Unrecognised shapes are assumed to cap, so an exotic range cannot make a
  // legitimate override look inert.
  return sawUnknown ? true : parts.length > 0;
}

// Cache the installed dependency index because each override queries it.
const installedRangesIndexes = new Map();

/** Installed dependency ranges by package, or null without an installed graph. */
function buildInstalledRangesIndex(root) {
  const pnpmDir = path.join(root, "node_modules", ".pnpm");
  if (!fs.existsSync(pnpmDir)) return null;

  const index = new Map();
  const visitManifestDir = (dir) => {
    let names;
    try {
      names = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of names) {
      if (!entry.isDirectory() && !entry.isSymbolicLink()) continue;
      if (entry.name.startsWith("@")) {
        visitManifestDir(path.join(dir, entry.name));
        continue;
      }
      const manifest = path.join(dir, entry.name, "package.json");
      let parsed;
      try {
        parsed = JSON.parse(fs.readFileSync(manifest, "utf8"));
      } catch {
        continue;
      }
      // Record one range per package name using dependency-field precedence.
      const declaredHere = new Map();
      for (const field of ["peerDependencies", "optionalDependencies", "dependencies"]) {
        for (const [name, range] of Object.entries(parsed[field] ?? {})) {
          // Skip the package's own entry so a package never justifies itself.
          if (parsed.name !== name) declaredHere.set(name, range);
        }
      }
      for (const [name, range] of declaredHere) {
        const ranges = index.get(name);
        if (ranges) ranges.push(range);
        else index.set(name, [range]);
      }
    }
  };

  for (const dir of fs.readdirSync(pnpmDir)) {
    visitManifestDir(path.join(pnpmDir, dir, "node_modules"));
  }
  return index;
}

/** Every version range the installed graph declares for `name`. */
function installedRangesFor(name, root) {
  if (!installedRangesIndexes.has(root)) {
    installedRangesIndexes.set(root, buildInstalledRangesIndex(root));
  }
  const index = installedRangesIndexes.get(root);
  if (index === null) return null;
  return index.get(name) ?? [];
}

/**
 * @param {(file: string) => string} read
 * @param {{ root?: string, today?: Date, installedRanges?: (name: string) => string[] | null }} [options]
 */
export function supplyChainSafetyFailures(read, options = {}) {
  const failures = [];
  const workspaceConfig = read("pnpm-workspace.yaml");
  const minimumReleaseAgeMatch = workspaceConfig.match(/^minimumReleaseAge:\s*(\d+)\s*$/m);
  const minimumReleaseAge = minimumReleaseAgeMatch
    ? Number.parseInt(minimumReleaseAgeMatch[1], 10)
    : 0;

  if (!Number.isFinite(minimumReleaseAge) || minimumReleaseAge < 1440) {
    failures.push(
      "pnpm-workspace.yaml must keep minimumReleaseAge at 1440 minutes or higher for the launch supply-chain quarantine.",
    );
  }

  const root = options.root ?? process.cwd();
  const today = options.today ?? new Date();
  const lookupRanges = options.installedRanges ?? ((name) => installedRangesFor(name, root));

  for (const entry of parseOverrideEntries(workspaceConfig)) {
    const justification = entry.comments.join("\n");
    const where = `pnpm-workspace.yaml:${entry.line} (overrides.${entry.name})`;
    const advisory = ADVISORY_PATTERN.exec(justification);
    const nonSecurity = justification.toLowerCase().includes(NON_SECURITY_MARKER);

    if (!advisory && !nonSecurity) {
      failures.push(
        `${where} needs a justification comment naming the advisory it patches (GHSA-/CVE-/RUSTSEC-) or starting with "${NON_SECURITY_MARKER}" and a reason. An unexplained override is indistinguishable from a stale one.`,
      );
    }

    const reviewLine = entry.comments.map((line) => REVIEW_PATTERN.exec(line)).find(Boolean);
    if (!reviewLine) {
      failures.push(
        `${where} needs a "# reviewed: YYYY-MM-DD" comment recording when a human last confirmed the override is still required.`,
      );
    } else {
      const reviewed = new Date(`${reviewLine[1]}T00:00:00Z`);
      if (Number.isNaN(reviewed.getTime())) {
        failures.push(`${where} has an unparseable reviewed date: ${reviewLine[1]}.`);
      } else {
        const ageDays = Math.floor((today.getTime() - reviewed.getTime()) / 86_400_000);
        if (ageDays > REVIEW_WARN_DAYS) {
          console.warn(
            `::warning::supply-chain: overrides.${entry.name} was last reviewed ${ageDays} days ago (threshold ${REVIEW_WARN_DAYS}). Confirm the dependency still needs it, then update the reviewed date in pnpm-workspace.yaml.`,
          );
        }
      }
    }

    const isExact = /^\d+\.\d+\.\d+/.test(entry.range);
    if (isExact && !justification.toLowerCase().includes(PINNED_EXACT_MARKER)) {
      failures.push(
        `${where} pins the exact version "${entry.range}". Renovate does not open PRs against pnpm-workspace.yaml, so an exact override here never advances. Use a floor ("^${entry.range}") or justify the freeze with a "${PINNED_EXACT_MARKER}" comment.`,
      );
    }

    const declaredRanges = lookupRanges(entry.name);

    // An override for a package nothing depends on is dead config whatever its
    // justification: there is no resolution left for the floor to rewrite. This
    // is the one inertness check a `non-security:` reason does not excuse, and
    // the gap that let `ws` and `sharp` outlive their dependents here.
    if (declaredRanges && declaredRanges.length === 0) {
      failures.push(
        `${where} overrides "${entry.name}", which nothing in the installed graph depends on. An override with nothing to rewrite is dead config that reads like protection. Remove it.`,
      );
    } else if (advisory && !nonSecurity) {
      // A security override is inert unless the graph caps the dependency below it.
      const floor = rangeFloor(entry.range);
      if (floor && declaredRanges) {
        const capping = declaredRanges.filter((range) => rangeCapsBelow(range, floor));
        if (capping.length === 0) {
          failures.push(
            `${where} is inert: every package in the installed graph already allows a version at or above "${entry.range}" (declared: ${[...new Set(declaredRanges)].sort().join(", ")}). A floor under an open range changes nothing. Remove the override, or replace the justification with a "${NON_SECURITY_MARKER}" reason if it is kept for deduplication.`,
          );
        }
      }
    }
  }

  return failures;
}
