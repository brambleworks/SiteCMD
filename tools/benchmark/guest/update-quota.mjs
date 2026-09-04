import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { evaluateQuota } from "../lib/workflow-quota.mjs";

const { directory, snapshot, billing } = JSON.parse(readFileSync(0, "utf8"));
if (!/^\/srv\/sitecmd-benchmark\/trials\/[a-f0-9]{24}$/.test(directory))
  throw new Error("Invalid trial directory");
if (!existsSync(`${directory}/quota-baseline.json`))
  throw new Error("Trial quota baseline not initialized");
const baseline = JSON.parse(readFileSync(`${directory}/quota-baseline.json`));
evaluateQuota(baseline, snapshot, billing);
writeFileSync(`${directory}/quota-current.next`, JSON.stringify(snapshot), { mode: 0o600 });
renameSync(`${directory}/quota-current.next`, `${directory}/quota-current.json`);
