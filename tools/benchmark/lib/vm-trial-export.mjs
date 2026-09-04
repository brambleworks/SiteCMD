import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { guestCommand } from "./vm-guest.mjs";
import { importTrial } from "./workflow-store.mjs";
import { artifactPath } from "./workflow-artifacts.mjs";

export function exportGuestTrial(run, plan, assignment, harness) {
  const exported = JSON.parse(
    guestCommand(["sudo", "node", `${harness.directory}/export-trial.mjs`], {
      input: JSON.stringify({ plan, assignment }),
      capture: true,
      timeout: 60000,
      maxBuffer: 384 * 1024 * 1024,
    }),
  );
  const destination = path.join(artifactPath(run, "inputs", { directory: true }), assignment.id);
  mkdirSync(destination, { mode: 0o700 });
  for (const [name, contents] of Object.entries(exported)) {
    if (
      !/^[a-zA-Z0-9][a-zA-Z0-9._/-]*$/.test(name) ||
      name.split("/").some((part) => !part || part === "." || part === "..")
    )
      throw new Error("Unsafe evidence filename");
    const target = path.join(destination, name);
    mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
    writeFileSync(target, Buffer.from(contents, "base64"), { flag: "wx", mode: 0o600 });
  }
  return importTrial(run, path.join(destination, "trial.json"));
}
