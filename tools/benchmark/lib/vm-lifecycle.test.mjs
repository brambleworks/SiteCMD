import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { assertPrivateState, startVm, stopVm } from "./vm-lifecycle.mjs";
import { createVmConfig, VM_NAME } from "./vm-config.mjs";
import { digest } from "./workflow-plan.mjs";

test("VM state rejects symlinked roots and global configuration that could restore host sharing", (t) => {
  const parent = mkdtempSync(path.join(tmpdir(), "sitecmd-vm-state-test-"));
  t.after(() => rmSync(parent, { recursive: true, force: true }));
  const actual = path.join(parent, "actual");
  const linked = path.join(parent, "linked");
  mkdirSync(actual);
  symlinkSync(actual, linked);
  assert.throws(() => assertPrivateState(linked), /real directory/);
  assert.doesNotThrow(() => assertPrivateState(actual));
  mkdirSync(path.join(actual, "lima", "_config"), { recursive: true });
  writeFileSync(path.join(actual, "lima", "_config", "override.yaml"), "mounts: []");
  assert.throws(() => assertPrivateState(actual), /global Lima configuration/);
});

test("stopping a running VM powers off the guest before stopping its host controller", () => {
  for (const states of [["Stopped"], ["Running", "Stopped"], ["Running", "Running", "Stopped"]]) {
    const remaining = [...states];
    const calls = [];
    stopVm("/unused", {
      run: (_root, args, options) => {
        calls.push(args);
        if (args[0] === "shell") assert.deepEqual(options.acceptedStatuses, [0, 255]);
        else if (args[0] === "stop") assert.deepEqual(options.acceptedStatuses, [0, 1]);
        else assert.equal(options.acceptedStatuses, undefined);
        return args[0] === "list" ? JSON.stringify({ status: remaining.shift() }) : "";
      },
    });
    assert.deepEqual(calls, [
      ["list", "--json", VM_NAME],
      ...(states[0] === "Running"
        ? [
            ["shell", "--workdir=/", VM_NAME, "sudo", "systemctl", "poweroff"],
            ["list", "--json", VM_NAME],
          ]
        : []),
      ...(states.length === 3
        ? [
            ["stop", "--tty=false", VM_NAME],
            ["list", "--json", VM_NAME],
          ]
        : []),
    ]);
  }
  assert.throws(
    () => stopVm("/unused", { run: () => JSON.stringify({ status: "Running" }) }),
    /did not stop/,
  );
});

test("starting a VM rejects changed source or instance configuration before launching it", (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "sitecmd-vm-frozen-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const instance = path.join(root, "lima", VM_NAME);
  mkdirSync(instance, { recursive: true });
  const config = Buffer.from("plain: true\n");
  writeFileSync(path.join(instance, "lima.yaml"), config);
  const receipt = path.join(instance, "sitecmd-config.json");
  for (const hashes of [
    { sourceSha256: "changed", instanceSha256: digest(config) },
    { sourceSha256: digest(createVmConfig()), instanceSha256: "changed" },
  ]) {
    writeFileSync(receipt, JSON.stringify(hashes));
    assert.throws(() => startVm(root), /VM configuration changed/);
  }
});
