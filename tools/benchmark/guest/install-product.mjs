import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  chownSync,
  copyFileSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Guest controller required");
const { commit, sourceSha256 } = JSON.parse(readFileSync(0, "utf8"));
if (!/^[a-f0-9]{40}$/.test(commit) || !/^[a-f0-9]{64}$/.test(sourceSha256))
  throw new Error("Invalid build identity");
const source = `/srv/sitecmd-benchmark/build/${commit}`;
const destination = `/opt/sitecmd-benchmark/products/${commit}`;
const hash = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");
if (existsSync(`${destination}/build.json`)) {
  const previous = JSON.parse(readFileSync(`${destination}/build.json`));
  if (previous.commit !== commit || previous.sourceSha256 !== sourceSha256 || !previous.version)
    throw new Error("Existing product receipt differs from this build");
  for (const kind of ["binary", "mcp", "cli"])
    if (hash(previous[kind]) !== previous[`${kind}Sha256`])
      throw new Error("Installed product changed; preserve its evidence before replacing it");
  console.log(JSON.stringify(previous));
  process.exit(0);
}
const target = `${source}/apps/desktop/src-tauri/target/release`;
const packages = readdirSync(`${target}/bundle/deb`).filter((name) => name.endsWith(".deb"));
if (packages.length !== 1) throw new Error("Expected one desktop Debian package");
const deb = `${target}/bundle/deb/${packages[0]}`;
const installed = spawnSync("dpkg", ["-i", deb], { encoding: "utf8", timeout: 120000 });
if (installed.status !== 0) throw new Error(`Desktop installation failed: ${installed.stderr}`);
const mcp = "/usr/lib/SiteCMD/sitecmd-mcp/sitecmd-mcp.mjs";
const binary = "/usr/bin/sitecmd";
const group = Number(spawnSync("id", ["-g", "sitecmd"], { encoding: "utf8" }).stdout.trim());
for (const file of [binary, path.dirname(mcp)]) {
  chownSync(file, 0, group);
  chmodSync(file, 0o750);
}
mkdirSync(destination, { recursive: true, mode: 0o700 });
for (const directory of [path.dirname(destination), destination]) {
  chownSync(directory, 0, group);
  chmodSync(directory, 0o750);
}
copyFileSync(`${target}/sitecmd_cli`, `${destination}/sitecmd_cli`);
chownSync(`${destination}/sitecmd_cli`, 0, group);
chmodSync(`${destination}/sitecmd_cli`, 0o750);
const receipt = {
  commit,
  sourceSha256,
  version: JSON.parse(readFileSync(`${source}/apps/desktop/package.json`)).version,
  binary,
  binarySha256: hash(binary),
  mcp,
  mcpSha256: hash(mcp),
  cli: `${destination}/sitecmd_cli`,
  cliSha256: hash(`${destination}/sitecmd_cli`),
  debSha256: hash(deb),
  environment: "Ubuntu 24.04 aarch64, isolated Lima VZ guest",
};
writeFileSync(`${destination}/build.json`, JSON.stringify(receipt), { flag: "wx", mode: 0o600 });
console.log(JSON.stringify(receipt));
