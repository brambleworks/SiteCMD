import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { digest } from "./workflow-plan.mjs";
import { guestCommand } from "./vm-guest.mjs";

export function deployHarness() {
  const directory = fileURLToPath(new URL("../guest/", import.meta.url));
  const files = {};
  for (const name of readdirSync(directory).filter((name) => /\.(mjs|py|apparmor)$/.test(name)))
    files[`guest/${name}`] = readFileSync(path.join(directory, name), "utf8");
  for (const name of [
    "workflow-contract",
    "workflow-plan",
    "workflow-quota",
    "workflow-usage",
    "workflow-results",
    "workflow-artifacts",
    "workflow-store",
    "workflow-pilot",
    "workflow-preflight",
    "trial-invocation",
  ])
    files[`lib/${name}.mjs`] = readFileSync(new URL(`./${name}.mjs`, import.meta.url), "utf8");
  files["pilot-policy.json"] = readFileSync(
    new URL("../pilot-policy.json", import.meta.url),
    "utf8",
  );
  for (const name of ["run-next.mjs", "prepare-calibration.mjs"])
    files[`host/${name}`] = readFileSync(new URL(`../${name}`, import.meta.url), "utf8");
  files["host/vm-harness.mjs"] = readFileSync(new URL("./vm-harness.mjs", import.meta.url), "utf8");
  files["host/vm-trial-export.mjs"] = readFileSync(
    new URL("./vm-trial-export.mjs", import.meta.url),
    "utf8",
  );
  const id = digest(files);
  const destination = `/srv/sitecmd-benchmark/controllers/${id}`;
  guestCommand(["sudo", "node", "--input-type=module", "-e", files["guest/install-harness.mjs"]], {
    input: JSON.stringify({ id, files }),
    capture: true,
  });
  return { id, directory: `${destination}/guest`, files };
}
