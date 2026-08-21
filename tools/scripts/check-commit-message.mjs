#!/usr/bin/env node

import fs from "node:fs";

import { commitMessageFailures } from "./lib/commit-message-rules.mjs";

function fail(message) {
  process.stderr.write(`Commit message check failed: ${message}\n`);
  process.exit(2);
}

const args = process.argv.slice(2);
let message;
let subjectOnly = false;

if (args[0] === "--file" && args[1]) {
  try {
    message = fs.readFileSync(args[1], "utf8");
  } catch (error) {
    fail(`cannot read ${args[1]}: ${error.message}`);
  }
} else if (args[0] === "--env" && args[1]) {
  message = process.env[args[1]];
  subjectOnly = true;
  if (message === undefined) fail(`environment variable ${args[1]} is not set`);
} else {
  fail("use --file <commit-message-file> or --env <variable-name>");
}

const failures = commitMessageFailures(message, { subjectOnly });
if (failures.length === 0) process.exit(0);

process.stderr.write("Commit message must be clear, concise, and written in plain English:\n");
for (const failure of failures) process.stderr.write(`- ${failure}\n`);
process.stderr.write("Example: Add verified site baselines\n");
process.exit(1);
