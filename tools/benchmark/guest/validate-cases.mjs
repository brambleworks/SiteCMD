import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { gradeCase } from "./calibration-grader.mjs";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Case validation requires the isolated guest controller");
const { cases, output } = JSON.parse(readFileSync(0, "utf8"));
if (!/^\/srv\/sitecmd-benchmark\/validation\/[a-f0-9]{32}$/.test(output))
  throw new Error("Invalid validation output path");
mkdirSync(output, { recursive: true, mode: 0o700 });
const results = [];
for (const item of cases) {
  if (!/^[a-z0-9-]+$/.test(item.id)) throw new Error("Invalid case identity");
  const runs = {};
  for (const variant of ["baseline", "reference"]) {
    const candidate = path.join(output, item.id, variant);
    mkdirSync(candidate, { recursive: true, mode: 0o755 });
    for (const [name, contents] of Object.entries(item[`${variant}Files`])) {
      if (
        name.startsWith("/") ||
        name.split("/").some((part) => !part || part === "." || part === "..")
      )
        throw new Error("Invalid source path");
      const target = path.join(candidate, name);
      mkdirSync(path.dirname(target), { recursive: true, mode: 0o755 });
      writeFileSync(target, contents, { flag: "wx", mode: 0o644 });
    }
    runs[variant] = Array.from({ length: 3 }, () => gradeCase(item, candidate));
  }
  const passed =
    runs.baseline.every(
      (run) => run.acceptancePass === (item.kind === "negative_control") && run.regressionsPass,
    ) && runs.reference.every((run) => run.acceptancePass && run.regressionsPass);
  results.push({ id: item.id, passed, runs });
}
const receipt = {
  capturedAt: new Date().toISOString(),
  output,
  executor: "guest independent behavioral grader",
  results,
};
writeFileSync(path.join(output, "grades.json"), JSON.stringify(receipt), {
  flag: "wx",
  mode: 0o600,
});
console.log(JSON.stringify(receipt));
