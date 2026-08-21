#!/usr/bin/env node

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
if (args[0] === "--") args.shift();
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");

const PAGE_ALIASES = new Map([
  ["dashboard", "dashboard"],
  ["today", "today"],
  ["search", "search-console"],
  ["seo", "search-console"],
  ["security", "security"],
  ["updates", "updates"],
  ["analytics", "analytics"],
  ["integrations", "integrations"],
  ["events", "events"],
  ["deploys", "deploys"],
  ["scans", "scans"],
  ["scan", "scans"],
  ["settings", "settings"],
]);

const VERIFY_ALIASES = {
  robots: { page: "search-console", focus: "seo.robots" },
  sitemap: { page: "search-console", focus: "seo.sitemap" },
  titles: { page: "search-console", focus: "seo.titles" },
  descriptions: { page: "search-console", focus: "seo.descriptions" },
  canonical: { page: "search-console", focus: "seo.canonical" },
  "structured-data": { page: "search-console", focus: "seo.structured_data" },
  structured: { page: "search-console", focus: "seo.structured_data" },
  noindex: { page: "search-console", focus: "seo.noindex" },
  https: { page: "security", focus: "sec.https" },
  ssl: { page: "security", focus: "sec.ssl_expiry" },
  headers: { page: "security", focus: "sec.headers" },
  hsts: { page: "security", focus: "sec.hsts" },
  "exposed-files": { page: "security", focus: "sec.exposed_files" },
};

function printHelp() {
  console.log(`SiteCMD CLI

Usage:
  sitecmd audit <path> [--format summary|json|markdown|review|github] [--fail-on critical|high|medium|low] [--output <file>]
  sitecmd today
  sitecmd dashboard
  sitecmd open <page> [--project <id>] [--url <url>] [--focus <key>] [--item <id>] [--lane pending-verification]
  sitecmd verify <target> [--project <id>] [--url <url>]

Examples:
  sitecmd audit .
  sitecmd audit . --format markdown --output guardrails.md
  sitecmd audit . --format review --output guardrails-review.md
  sitecmd audit . --format github --fail-on high
  sitecmd audit . --fail-on high
  sitecmd today
  sitecmd open dashboard --project 12 --url https://mysite.com
  sitecmd open search --project 12 --url https://mysite.com --focus seo.robots
  sitecmd verify robots --project 12 --url https://mysite.com
`);
}

function parseOptions(optionArgs) {
  const options = {};
  for (let index = 0; index < optionArgs.length; index += 1) {
    const token = optionArgs[index];
    if (!token.startsWith("--")) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const value = optionArgs[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for --${key}`);
    }
    options[key] = value;
    index += 1;
  }
  return options;
}

function buildUrl(target) {
  const url = new URL("sitecmd://open");
  url.searchParams.set("page", target.page);
  if (target.projectId) url.searchParams.set("projectId", String(target.projectId));
  if (target.url) url.searchParams.set("url", target.url);
  if (target.focus) url.searchParams.set("focus", target.focus);
  if (target.itemId) url.searchParams.set("itemId", target.itemId);
  if (target.lane) url.searchParams.set("lane", target.lane);
  if (target.restoreScan) url.searchParams.set("restoreScan", "1");
  return url.toString();
}

function openUrl(url) {
  if (process.platform === "darwin") {
    spawn("open", [url], { stdio: "inherit" });
    return;
  }
  if (process.platform === "win32") {
    spawn("cmd", ["/c", "start", "", url], { stdio: "inherit" });
    return;
  }
  spawn("xdg-open", [url], { stdio: "inherit" });
}

function runAuditCLI(auditArgs) {
  const child = spawn(
    "cargo",
    [
      "run",
      "--manifest-path",
      path.join(repoRoot, "apps", "desktop", "src-tauri", "crates", "cli", "Cargo.toml"),
      "--",
      "audit",
      ...auditArgs,
    ],
    {
      cwd: repoRoot,
      stdio: "inherit",
    },
  );

  child.on("exit", (code) => {
    process.exit(code ?? 1);
  });
  child.on("error", (error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}

function parseCommonTargetOptions(options) {
  return {
    projectId: options.project ? Number.parseInt(options.project, 10) : undefined,
    url: options.url,
    focus: options.focus,
    itemId: options.item,
    lane: options.lane,
  };
}

try {
  if (args.length === 0 || args[0] === "help" || args[0] === "--help" || args[0] === "-h") {
    printHelp();
    process.exit(0);
  }

  let target;
  const [command, ...rest] = args;

  if (command === "audit") {
    if (rest.length === 0) throw new Error("Missing project path for audit.");
    runAuditCLI(rest);
  } else if (command === "open") {
    const pageArg = rest[0];
    if (!pageArg) throw new Error("Missing page name.");
    const page = PAGE_ALIASES.get(pageArg);
    if (!page) throw new Error(`Unknown page: ${pageArg}`);
    const options = parseOptions(rest.slice(1));
    target = {
      page,
      ...parseCommonTargetOptions(options),
    };
  } else if (command === "verify") {
    const verifyArg = rest[0];
    if (!verifyArg) throw new Error("Missing verify target.");
    const base = VERIFY_ALIASES[verifyArg];
    if (!base) throw new Error(`Unknown verify target: ${verifyArg}`);
    const options = parseOptions(rest.slice(1));
    target = {
      ...base,
      ...parseCommonTargetOptions(options),
    };
  } else {
    const page = PAGE_ALIASES.get(command);
    if (!page) throw new Error(`Unknown command: ${command}`);
    const options = parseOptions(rest);
    target = {
      page,
      ...parseCommonTargetOptions(options),
    };
  }

  if (target) {
    const url = buildUrl(target);
    openUrl(url);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  console.error("");
  printHelp();
  process.exit(1);
}
