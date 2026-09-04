import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Harness installation requires the isolated guest controller");
const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const { id, files } = JSON.parse(Buffer.concat(chunks));
if (!/^[a-f0-9]{64}$/.test(id)) throw new Error("Invalid harness identity");
const directory = `/srv/sitecmd-benchmark/controllers/${id}`;
mkdirSync("/srv/sitecmd-benchmark/controllers", { recursive: true, mode: 0o700 });
mkdirSync(directory, { recursive: true, mode: 0o700 });
for (const [name, contents] of Object.entries(files)) {
  if (
    !/^[a-zA-Z0-9_./-]+$/.test(name) ||
    name.split("/").some((part) => !part || part === "." || part === "..")
  )
    throw new Error("Invalid harness path");
  const target = path.join(directory, name);
  mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
  if (existsSync(target)) {
    if (readFileSync(target, "utf8") !== contents)
      throw new Error("Installed harness was modified");
  } else {
    writeFileSync(target, contents, { flag: "wx", mode: 0o644 });
  }
}
console.log(directory);
