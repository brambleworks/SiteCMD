import { readFileSync } from "node:fs";
import { collectEvidence } from "../lib/workflow-artifacts.mjs";

const { plan, assignment } = JSON.parse(readFileSync(0, "utf8"));
if (!/^[a-f0-9]{24}$/.test(assignment.id)) throw new Error("Invalid trial identity");
const directory = `/srv/sitecmd-benchmark/trials/${assignment.id}`;
const record = JSON.parse(readFileSync(`${directory}/trial.json`));
const files = collectEvidence(record, assignment, plan, directory);
files.set("trial.json", Buffer.from(JSON.stringify(record)));
console.log(
  JSON.stringify(
    Object.fromEntries([...files].map(([name, bytes]) => [name, bytes.toString("base64")])),
  ),
);
