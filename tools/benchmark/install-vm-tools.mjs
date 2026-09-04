import { readFileSync } from "node:fs";
import { guestCommand } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";

guestCommand(["sudo", "bash", "-s"], {
  input: readFileSync(new URL("./guest/install-tools.sh", import.meta.url)),
  timeout: 1800000,
});
const harness = deployHarness();
guestCommand(["sudo", "node", `${harness.directory}/install-isolation.mjs`]);
