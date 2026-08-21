#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { brokerOnlyCommands } from "../lib/guardrail-invoke-acl-rules.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const RUST_SRC = path.join(ROOT, "apps/desktop/src-tauri/src");
const BUILD_RS = path.join(ROOT, "apps/desktop/src-tauri/build.rs");
const LIB_RS = path.join(ROOT, "apps/desktop/src-tauri/src/lib.rs");
const CAPS_DIR = path.join(ROOT, "apps/desktop/src-tauri/capabilities");
const FRONTEND_ROOTS = [path.join(ROOT, "apps/desktop/src")];
const SKIP = new Set(["node_modules", "dist", "target", ".next"]);

function read(p) {
  return fs.readFileSync(p, "utf8");
}

function walk(dir, exts, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP.has(e.name) || e.name.startsWith(".")) continue;
    const abs = path.join(dir, e.name);
    if (e.isDirectory()) walk(abs, exts, out);
    else if (e.isFile() && exts.has(path.extname(e.name))) out.push(abs);
  }
  return out;
}

function findRustCommands() {
  const files = walk(RUST_SRC, new Set([".rs"]));
  const cmds = new Map();
  // Match complete attributes so comments and strings cannot declare commands.
  const bareAnn = /^\s*#\[tauri::command(?:\([^)]*\))?\]\s*$/;
  const isCfgAttrCommand = (line) => {
    const t = line.trim();
    return t.startsWith("#[cfg_attr(") && t.endsWith(")]") && t.includes("tauri::command");
  };
  const sig = /^\s*(?:pub(?:\s*\([^)]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)/;
  for (const f of files) {
    if (f.includes("/tests/") || /\/tests?\.rs$/.test(f)) continue;
    const lines = read(f).split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (!bareAnn.test(lines[i]) && !isCfgAttrCommand(lines[i])) continue;
      // Allow up to 8 intervening attributes (e.g. #[tracing::instrument]) before fn.
      for (let j = i + 1; j < Math.min(i + 9, lines.length); j++) {
        const m = sig.exec(lines[j]);
        if (m) {
          const name = m[1];
          const rel = path.relative(ROOT, f).replaceAll(path.sep, "/");
          if (!cmds.has(name)) cmds.set(name, []);
          cmds.get(name).push({ file: rel, line: j + 1 });
          break;
        }
      }
    }
  }
  return cmds;
}

function parseAppCommands() {
  const text = read(BUILD_RS);
  // Match `APP_COMMANDS: &[&str] = &[ ... ];`. The array literal opens after
  // `= &[`, not the `&[&str]` that appears in the type ascription.
  const m = text.match(/APP_COMMANDS\s*:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/);
  if (!m) return new Set();
  const set = new Set();
  for (const x of m[1].matchAll(/"([a-z_][a-z0-9_]*)"/g)) set.add(x[1]);
  return set;
}

function parseInvokeHandler() {
  const text = read(LIB_RS);
  const start = text.indexOf("invoke_handler(tauri::generate_handler!");
  if (start === -1) return new Set();
  const open = text.indexOf("[", start);
  let depth = 0,
    end = open;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "[") depth++;
    else if (text[i] === "]") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  const block = text.slice(open + 1, end);
  const set = new Set();
  // Capture the trailing identifier from each Rust module path.
  for (const m of block.matchAll(/(?:[a-z_][a-z0-9_]*::)+([a-z_][a-z0-9_]*)/g)) set.add(m[1]);
  return set;
}

function parseCapabilities() {
  const result = new Map();
  // Fail closed when capability sources cannot be read.
  if (!fs.existsSync(CAPS_DIR)) {
    console.error(`tauri-command-surface: capabilities dir missing: ${CAPS_DIR}`);
    process.exit(2);
  }
  for (const e of fs.readdirSync(CAPS_DIR)) {
    if (!e.endsWith(".json")) continue;
    let parsed;
    try {
      parsed = JSON.parse(read(path.join(CAPS_DIR, e)));
    } catch (error) {
      console.error(`tauri-command-surface: cannot parse capability ${e}: ${error}`);
      process.exit(2);
    }
    const perms = Array.isArray(parsed.permissions) ? parsed.permissions : [];
    for (const p of perms) {
      if (typeof p !== "string") continue;
      const m = /^allow-([a-z][a-z0-9-]*)$/.exec(p);
      if (!m) continue;
      const cmd = m[1].replaceAll("-", "_");
      if (!result.has(cmd)) result.set(cmd, []);
      result.get(cmd).push(e);
    }
  }
  return result;
}

function findFrontendInvokes() {
  const exts = new Set([".ts", ".tsx"]);
  const re = /\binvoke(?:<[^>]*>)?\s*\(\s*["'`]([a-z_][a-z0-9_]*)["'`]/g;
  const result = new Map();
  for (const root of FRONTEND_ROOTS) {
    if (!fs.existsSync(root)) continue;
    for (const f of walk(root, exts)) {
      const text = read(f);
      for (const m of text.matchAll(re)) {
        const rel = path.relative(ROOT, f).replaceAll(path.sep, "/");
        if (!result.has(m[1])) result.set(m[1], []);
        result.get(m[1]).push(rel);
      }
    }
  }
  return result;
}

function parseArgs(argv) {
  const a = { json: false, check: false };
  for (const x of argv) {
    if (x === "--json") a.json = true;
    else if (x === "--check") a.check = true;
  }
  return a;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const rust = findRustCommands();
  const app = parseAppCommands();
  const handler = parseInvokeHandler();
  const caps = parseCapabilities();
  const fe = findFrontendInvokes();

  const issues = [];
  for (const name of rust.keys()) {
    if (!app.has(name)) issues.push({ kind: "missing_app_commands", command: name });
    if (!handler.has(name)) issues.push({ kind: "missing_invoke_handler", command: name });
  }
  for (const name of app)
    if (!rust.has(name)) issues.push({ kind: "app_commands_no_rust_fn", command: name });
  for (const name of handler)
    if (!rust.has(name)) issues.push({ kind: "invoke_handler_no_rust_fn", command: name });
  // Reject grants that no longer name a Rust command.
  for (const name of caps.keys())
    if (!rust.has(name)) issues.push({ kind: "capability_no_rust_fn", command: name });

  const brokerRouted = brokerOnlyCommands((rel) => read(path.join(ROOT, rel)));
  const orphaned = [];
  for (const name of fe.keys())
    if (!handler.has(name) && !rust.has(name) && !brokerRouted.has(name)) orphaned.push(name);

  if (args.json) {
    process.stdout.write(
      JSON.stringify(
        {
          counts: {
            rustCommands: rust.size,
            appCommands: app.size,
            invokeHandler: handler.size,
            capabilityRefs: caps.size,
            frontendInvokeNames: fe.size,
          },
          issues,
          orphanedFrontend: orphaned,
        },
        null,
        2,
      ) + "\n",
    );
    if (args.check && (issues.length || orphaned.length)) process.exit(1);
    return;
  }

  process.stdout.write("Tauri command surface\n=====================\n\n");
  process.stdout.write(`Rust #[tauri::command] count:        ${rust.size}\n`);
  process.stdout.write(`build.rs APP_COMMANDS count:         ${app.size}\n`);
  process.stdout.write(`lib.rs invoke_handler! count:        ${handler.size}\n`);
  process.stdout.write(`Capability allow-* unique commands:  ${caps.size}\n`);
  process.stdout.write(`Frontend invoke("...") names:        ${fe.size}\n\n`);

  if (issues.length === 0) process.stdout.write("No registration drift detected.\n");
  else {
    process.stdout.write(`Registration issues (${issues.length}):\n`);
    for (const i of issues) process.stdout.write(`  ${i.kind.padEnd(28)}  ${i.command}\n`);
  }
  if (orphaned.length) {
    process.stdout.write(
      `\nFrontend invokes without matching Rust command (${orphaned.length}):\n`,
    );
    for (const n of orphaned) process.stdout.write(`  ${n}\n`);
  }
  process.stdout.write(`\n--- Top 5 commands by capability scope coverage ---\n`);
  const scoped = [...caps.entries()].sort((a, b) => b[1].length - a[1].length).slice(0, 5);
  for (const [cmd, files] of scoped)
    process.stdout.write(`  ${cmd.padEnd(40)} -> ${files.join(", ")}\n`);

  if (args.check && (issues.length || orphaned.length)) process.exit(1);
}

main();
