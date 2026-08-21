#!/usr/bin/env node

import { execFileSync, spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { CATALOG_DEV_ENV } from "./lib/catalog-dev-env.mjs";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function pidsMatching(pattern) {
  try {
    return execFileSync("pgrep", ["-f", pattern], { encoding: "utf8" })
      .split("\n")
      .map((line) => Number.parseInt(line, 10))
      .filter((pid) => Number.isInteger(pid) && pid > 0 && pid !== process.pid);
  } catch {
    return [];
  }
}

function pidsOnPort(port) {
  try {
    return execFileSync("lsof", ["-ti", `tcp:${port}`, "-sTCP:LISTEN"], { encoding: "utf8" })
      .split("\n")
      .map((line) => Number.parseInt(line, 10))
      .filter((pid) => Number.isInteger(pid) && pid > 0);
  } catch {
    return [];
  }
}

function belongsToThisCheckout(pid) {
  try {
    const command = execFileSync("ps", ["-o", "command=", "-p", String(pid)], {
      encoding: "utf8",
    }).trim();
    return command.includes(REPO_ROOT);
  } catch {
    // An unverified process is never safe to kill.
    return false;
  }
}

const DEV_PORTS = [5173, 5174];

function killStaleDevStack() {
  if (process.platform === "win32") return;

  // Only stop candidates whose command line confirms this checkout.
  const candidates = new Set([
    ...pidsMatching("tauri dev --config src-tauri/tauri"),
    ...pidsMatching("src-tauri/target/debug"),
    ...DEV_PORTS.flatMap(pidsOnPort),
  ]);

  for (const pid of candidates) {
    if (!belongsToThisCheckout(pid)) {
      console.log(
        `Port or process ${pid} is held by something outside this checkout; leaving it alone.`,
      );
      continue;
    }
    try {
      process.kill(pid, "SIGKILL");
      console.log(`Stopped stale dev process ${pid}.`);
    } catch {
      // Already gone between listing and killing; nothing to do.
    }
  }
}

async function requireFreePorts() {
  const deadline = Date.now() + 3000;
  for (;;) {
    const holders = DEV_PORTS.flatMap(pidsOnPort);
    if (holders.length === 0) return;
    if (Date.now() > deadline) {
      for (const pid of new Set(holders)) {
        console.error(`Port still held by pid ${pid} after SIGKILL; inspect it with: ps -p ${pid}`);
      }
      process.exit(1);
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
}

killStaleDevStack();
await requireFreePorts();

const child = spawn(
  "pnpm",
  [
    "--filter",
    "@sitecmd/desktop",
    "exec",
    "tauri",
    "dev",
    "--config",
    "src-tauri/tauri.dev.conf.json",
  ],
  {
    stdio: "inherit",
    shell: process.platform === "win32",
    env: { ...CATALOG_DEV_ENV, ...process.env },
  },
);

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
