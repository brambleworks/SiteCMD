import assert from "node:assert/strict";
import { test } from "node:test";
import { createVmConfig, vmEnvironment } from "./vm-config.mjs";

test("the VM has bounded resources and no host filesystem or credential integrations", () => {
  const config = createVmConfig();
  assert.equal(config.vmType, "vz");
  assert.equal(config.plain, true);
  assert.equal(config.cpus, 4);
  assert.equal(config.memory, "6GiB");
  assert.equal(config.disk, "32GiB");
  assert.deepEqual(config.mounts, []);
  assert.deepEqual(config.portForwards, []);
  assert.equal(config.ssh.forwardAgent, false);
  assert.equal(config.ssh.loadDotSSHPubKeys, false);
  assert.equal(config.propagateProxyEnv, false);
  assert.equal(config.user.name, "benchadmin");
  assert.equal(
    config.provision.find((item) => item.path === "/opt/sitecmd-benchmark/verify.sh").permissions,
    "755",
  );
  assert.match(config.images[0].digest, /^sha256:[a-f0-9]{64}$/);
  const env = vmEnvironment("/private/tmp/sitecmd-vm", {
    HOME: "/Users/dev",
    PATH: "/usr/bin:/bin",
    SSH_AUTH_SOCK: "secret-socket",
    ANTHROPIC_API_KEY: "secret",
    OPENAI_API_KEY: "secret",
    HTTP_PROXY: "private-proxy",
    LIMA_HOME: "/existing-vms",
  });
  assert.equal(env.HOME, "/Users/dev");
  assert.equal(env.LIMA_HOME, "/private/tmp/sitecmd-vm/lima");
  for (const key of ["SSH_AUTH_SOCK", "ANTHROPIC_API_KEY", "OPENAI_API_KEY", "HTTP_PROXY"])
    assert.equal(env[key], undefined);
});
