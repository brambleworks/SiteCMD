#!/usr/bin/env node
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { sourceSnapshot } from "./lib/vm-source.mjs";
import { guestCommand, workRoot } from "./lib/vm-guest.mjs";
import { deployHarness } from "./lib/vm-harness.mjs";
import { writeNewJson } from "./lib/workflow-store.mjs";
import { digest } from "./lib/workflow-plan.mjs";

const repository = fileURLToPath(new URL("../../", import.meta.url));
const snapshot = sourceSnapshot(repository);
const destination = `/srv/sitecmd-benchmark/build/${snapshot.commit}`;
if (process.argv.slice(2).some((arg) => arg !== "--install-existing"))
  throw new Error("Usage: build-vm.mjs [--install-existing]");
if (!process.argv.includes("--install-existing")) {
  console.log(`Building committed SiteCMD ${snapshot.commit}; archive SHA-256 ${snapshot.sha256}`);
  guestCommand(["sudo", "-u", "builder", "mkdir", "-m", "700", destination]);
  guestCommand(["sudo", "-u", "builder", "tar", "--no-same-owner", "-xf", "-", "-C", destination], {
    input: snapshot.archive,
  });
  guestCommand(["sudo", "-u", "builder", "bash", "-s", "--", destination], {
    input: readFileSync(new URL("./guest/build-product.sh", import.meta.url), "utf8"),
    timeout: 7200000,
  });
}
const harness = deployHarness();
const receipt = JSON.parse(
  guestCommand(["sudo", "node", `${harness.directory}/install-product.mjs`], {
    input: JSON.stringify({ commit: snapshot.commit, sourceSha256: snapshot.sha256 }),
    capture: true,
    timeout: 180000,
  }),
);
const receiptPath = `${workRoot}/product-${snapshot.commit}.json`;
if (existsSync(receiptPath)) {
  if (digest(JSON.parse(readFileSync(receiptPath))) !== digest(receipt))
    throw new Error("Host and guest product receipts differ");
} else writeNewJson(receiptPath, receipt);
console.log(`Installed committed product. Receipt: ${receiptPath}`);
