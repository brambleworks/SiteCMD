import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";

const id = process.argv[2];
if (process.getuid() !== 0 || !/^[a-f0-9]{24}$/.test(id))
  throw new Error("Cancellation requires a guest controller and exact trial identity");
writeFileSync(`/run/sitecmd-benchmark-cancel-${id}`, "Operator requested cancellation\n", {
  mode: 0o600,
});
spawnSync("systemctl", ["stop", `sitecmd-agent-${id}`], { timeout: 15000, stdio: "ignore" });
