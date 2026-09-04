import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { runtimeLock } from "./vm-config.mjs";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

export async function downloadVerified(artifact, destination, fetcher = fetch) {
  if (!artifact.url.startsWith("https://") || !/^[a-f0-9]{64}$/.test(artifact.sha256))
    throw new Error("An HTTPS artifact URL and pinned SHA-256 are required");
  const response = await fetcher(artifact.url, { signal: AbortSignal.timeout(180000) });
  if (!response.ok || !response.body)
    throw new Error(`Runtime download failed: ${response.status}`);
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 100 * 1024 * 1024) throw new Error("Runtime download exceeds 100 MiB");
    chunks.push(chunk);
  }
  const bytes = Buffer.concat(chunks);
  if (sha256(bytes) !== artifact.sha256) throw new Error("Runtime download checksum mismatch");
  writeFileSync(destination, bytes, { flag: "wx", mode: 0o600 });
}

export function runtimePath(workRoot) {
  return path.join(workRoot, "vm-runtime", `lima-${runtimeLock.lima.version}`, "bin", "limactl");
}

export function verifyRuntime(workRoot) {
  const binary = runtimePath(workRoot);
  const receipt = JSON.parse(
    readFileSync(path.join(path.dirname(binary), "..", "sitecmd-runtime.json"), "utf8"),
  );
  if (
    receipt.archiveSha256 !== runtimeLock.lima.sha256 ||
    receipt.binarySha256 !== sha256(readFileSync(binary))
  )
    throw new Error("Installed Lima runtime changed; inspect it before continuing");
  return binary;
}

export async function installRuntime(workRoot) {
  if (process.platform !== "darwin" || process.arch !== "arm64")
    throw new Error("This VM bootstrap currently supports Apple Silicon macOS only");
  if (existsSync(runtimePath(workRoot))) return verifyRuntime(workRoot);
  const parent = path.join(workRoot, "vm-runtime");
  mkdirSync(parent, { recursive: true, mode: 0o700 });
  const staging = mkdtempSync(path.join(parent, "install-"));
  const archive = path.join(staging, "lima.tar.gz");
  console.log(`Downloading checksum-pinned Lima ${runtimeLock.lima.version}`);
  await downloadVerified(runtimeLock.lima, archive);
  const extracted = path.join(staging, "runtime");
  mkdirSync(extracted, { mode: 0o700 });
  const result = spawnSync("tar", ["-xzf", archive, "-C", extracted], {
    stdio: "inherit",
    timeout: 60000,
  });
  if (result.status !== 0) throw new Error("Lima archive extraction failed");
  writeFileSync(
    path.join(extracted, "sitecmd-runtime.json"),
    JSON.stringify({
      archiveSha256: runtimeLock.lima.sha256,
      binarySha256: sha256(readFileSync(path.join(extracted, "bin", "limactl"))),
    }),
    { flag: "wx", mode: 0o600 },
  );
  renameSync(extracted, path.join(parent, `lima-${runtimeLock.lima.version}`));
  return verifyRuntime(workRoot);
}
