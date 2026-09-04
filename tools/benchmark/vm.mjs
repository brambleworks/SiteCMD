#!/usr/bin/env node
import { fileURLToPath } from "node:url";
import { setupVm, startVm, stopVm, vmStatus, verifyVm, vmShell } from "./lib/vm-lifecycle.mjs";
import { guestCommand } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";

const workRoot = fileURLToPath(new URL("./.work/", import.meta.url));
const commands = {
  setup: setupVm,
  start: startVm,
  stop: stopVm,
  status: (root) => console.log(JSON.stringify(vmStatus(root), null, 2)),
  verify: verifyVm,
  shell: vmShell,
  doctor: () => {
    const harness = deployHarness();
    const result = JSON.parse(
      guestCommand(["sudo", "node", `${harness.directory}/doctor.mjs`], { capture: true }),
    );
    console.log(JSON.stringify(result, null, 2));
    if (!result.subscriptionAccountsVerified) process.exitCode = 2;
  },
};
const [command, ...extra] = process.argv.slice(2);
try {
  if (!command || command === "--help") {
    console.log(
      "Usage: pnpm benchmark:vm <setup|start|stop|status|verify|shell|doctor>\nControls only the dedicated SiteCMD benchmark VM. No agent trials, logins, or host mounts.",
    );
  } else {
    if (!commands[command] || extra.length)
      throw new Error("Unknown VM command or extra arguments");
    await commands[command](workRoot);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
