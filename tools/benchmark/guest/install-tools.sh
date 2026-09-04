#!/bin/bash
set -euo pipefail
test "$(id -u)" = 0
test "$(uname -m)" = aarch64
apt-get update -qq
apt-get install -y --no-install-recommends webkit2gtk-driver bubblewrap ripgrep
install -d -m 0755 /opt/sitecmd-benchmark/agents
npm install --prefix /opt/sitecmd-benchmark/agents --save-exact \
  @openai/codex@0.153.0-alpha.5 @anthropic-ai/claude-code@2.1.260
node --input-type=module -e '
  import { readFileSync } from "node:fs";
  const lock = JSON.parse(readFileSync("/opt/sitecmd-benchmark/agents/package-lock.json"));
  const expected = {
    "@openai/codex": "sha512-yXmgMVUDYLBWENhzW9pMelg5MyveXisr1d4g518E47JpECpxrG8EsnLutpl29lkODJMxG5tisY4tIlybWe8vSA==",
    "@anthropic-ai/claude-code": "sha512-Arqg8BvlOehmC3QdACN2WKshqqWQVMo+5NwG22aiJbw7M6S1LM7E2pA2MjD8BS5P5EwZVkh2eKUmC6k7pVUqSQ==",
  };
  for (const [name, integrity] of Object.entries(expected)) {
    if (lock.packages[`node_modules/${name}`]?.integrity !== integrity)
      throw new Error(`Agent package integrity mismatch: ${name}`);
  }
'
ln -sf /opt/sitecmd-benchmark/agents/node_modules/.bin/codex /usr/local/bin/codex
ln -sf /opt/sitecmd-benchmark/agents/node_modules/.bin/claude /usr/local/bin/claude
sudo -u builder cargo install tauri-driver --version 2.0.6 --locked --jobs 1
build_home="$(getent passwd builder | cut -d: -f6)"
install -m 0755 "$build_home/.cargo/bin/tauri-driver" /usr/local/bin/tauri-driver
sudo -u runner codex --version
sudo -u runner env DISABLE_AUTOUPDATER=1 claude --version
sudo -u builder cargo install --list
tauri-driver --help
dpkg-query --show --showformat='${Package} ${Version}\n' webkit2gtk-driver bubblewrap
bwrap --version
