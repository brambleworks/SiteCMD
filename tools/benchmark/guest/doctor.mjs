import { spawnSync } from "node:child_process";
import { probeAgentAccounts } from "../lib/workflow-preflight.mjs";

const result = probeAgentAccounts({
  run: (command, args, options) => spawnSync("sudo", ["-u", "runner", command, ...args], options),
});
console.log(JSON.stringify(result));
