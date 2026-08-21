#!/usr/bin/env node

import { spawn } from "node:child_process";
import {
  attachRiskAcknowledged,
  buildAttachPreflightFailureMessage,
  waitForHealthyDevServer,
} from "./tauri-attach-lib.mjs";
import { CATALOG_DEV_ENV } from "./lib/catalog-dev-env.mjs";

const DEV_URL = process.env.SITECMD_ATTACH_DEV_URL || "http://127.0.0.1:5173";
const ATTACH_TIMEOUT_MS = Number.parseInt(process.env.SITECMD_ATTACH_TIMEOUT_MS || "", 10) || 15000;
const ATTACH_POLL_MS = Number.parseInt(process.env.SITECMD_ATTACH_POLL_MS || "", 10) || 350;

if (!attachRiskAcknowledged()) {
  console.error(
    "Refusing privileged Tauri attach without SITECMD_ALLOW_PRIVILEGED_ATTACH=1. Attach mode trusts whatever process controls the configured localhost dev port with the main renderer's development permissions.",
  );
  console.error(
    "Prefer `pnpm tauri:dev`, which starts Vite itself with strict-port protection. Use attach mode only for a dev server you started and verified.",
  );
  process.exit(1);
}

function runAttach() {
  const child = spawn(
    "pnpm",
    [
      "--filter",
      "@sitecmd/desktop",
      "exec",
      "tauri",
      "dev",
      "--config",
      "src-tauri/tauri.attach.conf.json",
    ],
    {
      stdio: "inherit",
      shell: process.platform === "win32",
      // Caller overrides win over development catalog defaults.
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
}

const healthy = await waitForHealthyDevServer(DEV_URL, {
  timeoutMs: ATTACH_TIMEOUT_MS,
  pollMs: ATTACH_POLL_MS,
});

if (!healthy) {
  for (const line of buildAttachPreflightFailureMessage(DEV_URL)) {
    console.error(line);
  }
  process.exit(1);
}

runAttach();
