import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const SCRIPT = path.join(ROOT, "tools/scripts/audit/check-codeql.mjs");
const WORK_PREFIX = "sitecmd-codeql-";

let scratch;
let fakeBin;
let argsLog;

/**
 * A codeql stand-in, so these tests exercise the gate's own logic without the
 * several minutes and gigabyte of database a real analysis costs. It records
 * every invocation, creates the database directory it is asked for, and writes
 * an empty SARIF result set.
 */
function installFakeCodeql() {
  const stub = path.join(fakeBin, "codeql");
  fs.writeFileSync(
    stub,
    [
      "#!/usr/bin/env node",
      'const fs = require("node:fs");',
      "const args = process.argv.slice(2);",
      `fs.appendFileSync(${JSON.stringify(argsLog)}, args.join(" ") + "\\n");`,
      'if (args[0] === "version") { console.log("0.0.0-fake"); process.exit(0); }',
      'if (args[1] === "create") { fs.mkdirSync(args[2], { recursive: true }); process.exit(0); }',
      'if (args[1] === "analyze") {',
      '  const out = args.find((a) => a.startsWith("--output="));',
      '  fs.writeFileSync(out.slice("--output=".length), JSON.stringify({ runs: [] }));',
      "}",
      "process.exit(0);",
    ].join("\n"),
  );
  fs.chmodSync(stub, 0o755);
}

/** Age a directory past the staleness cutoff without waiting an hour. */
function makeOld(directory) {
  const old = new Date(Date.now() - 2 * 60 * 60 * 1000);
  fs.utimesSync(directory, old, old);
}

function seedWorkDirectory(name) {
  const directory = path.join(scratch, name);
  fs.mkdirSync(path.join(directory, "db"), { recursive: true });
  makeOld(directory);
  return directory;
}

function runGate(env = {}) {
  return spawnSync(process.execPath, [SCRIPT], {
    cwd: ROOT,
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${fakeBin}${path.delimiter}${process.env.PATH}`,
      TMPDIR: scratch,
      SITECMD_CODEQL_BASE: "HEAD",
      ...env,
    },
  });
}

beforeEach(() => {
  scratch = fs.mkdtempSync(path.join(os.tmpdir(), "check-codeql-test-"));
  fakeBin = path.join(scratch, "bin");
  fs.mkdirSync(fakeBin);
  argsLog = path.join(scratch, "codeql-args.log");
  installFakeCodeql();
});

afterEach(() => {
  fs.rmSync(scratch, { recursive: true, force: true });
});

describe("check-codeql stale database sweep", () => {
  it("removes an abandoned database even when there is nothing to analyze", () => {
    const abandoned = seedWorkDirectory(`${WORK_PREFIX}999999-abandoned`);

    const result = runGate();

    expect(result.stdout).toContain("no added lines");
    expect(fs.existsSync(abandoned)).toBe(false);
  });

  it("keeps a directory whose owner is still running, however old it looks", () => {
    // CodeQL writes below work/db, so a live run's parent mtime stops advancing
    // and the age test alone would eventually mistake it for abandoned work.
    const live = seedWorkDirectory(`${WORK_PREFIX}${process.pid}-live`);

    runGate();

    expect(fs.existsSync(live)).toBe(true);
  });

  it("leaves a recent directory alone", () => {
    const directory = path.join(scratch, `${WORK_PREFIX}999999-recent`);
    fs.mkdirSync(directory, { recursive: true });

    runGate();

    expect(fs.existsSync(directory)).toBe(true);
  });
});

describe("check-codeql memory budget", () => {
  const analyzed = { SITECMD_CODEQL_BASE: "HEAD~1" };

  function ramFlags() {
    return fs
      .readFileSync(argsLog, "utf8")
      .split("\n")
      .flatMap((line) => line.split(" ").filter((arg) => arg.startsWith("--ram=")));
  }

  it("passes the default budget when no override is set", () => {
    const result = runGate(analyzed);

    expect(result.status).toBe(0);
    expect(ramFlags()).toContain("--ram=4096");
  });

  it.each(["", "abc", "0", "-1", "4.5"])(
    "falls back to the default rather than sending %j to codeql",
    (value) => {
      // CodeQL requires a positive integer, and an unchecked Number() turns
      // these into --ram=0 or --ram=NaN, which fails the analysis.
      runGate({ ...analyzed, SITECMD_CODEQL_RAM: value });

      expect(ramFlags()).toContain("--ram=4096");
    },
  );

  it("honours a valid override", () => {
    runGate({ ...analyzed, SITECMD_CODEQL_RAM: "8192" });

    expect(ramFlags()).toContain("--ram=8192");
  });

  it("removes its work directory once the analysis is clean", () => {
    const result = runGate(analyzed);

    expect(result.status).toBe(0);
    const left = fs.readdirSync(scratch).filter((name) => name.startsWith(WORK_PREFIX));
    expect(left).toEqual([]);
  });
});

describe("check-codeql cleanup discipline", () => {
  it("exits through die(), so no path skips the finally that frees the database", () => {
    // process.exit inside the try block abandons a database of several hundred
    // megabytes. die() removes the directory first; the alert path sets
    // process.exitCode instead so the finally still runs.
    const source = fs.readFileSync(SCRIPT, "utf8");
    const callers = source
      .split("\n")
      .map((line, index) => ({ line: line.trim(), number: index + 1 }))
      .filter((entry) => entry.line.startsWith("process.exit("));

    expect(callers).toHaveLength(1);
    const die = source.slice(source.indexOf("function die("), source.indexOf("/** Line ranges"));
    expect(die).toContain("process.exit(1)");
  });
});
