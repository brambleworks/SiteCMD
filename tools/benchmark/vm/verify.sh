#!/bin/bash
set -euo pipefail

test "$(uname -s)" = Linux
test "$(uname -m)" = aarch64
systemd-detect-virt --quiet
if findmnt -rn -t virtiofs,9p,fuse.sshfs; then
  echo "Host filesystem sharing is enabled" >&2
  exit 1
fi
test ! -e /Users
test ! -S /run/host-services/ssh-auth.sock
test -z "${SSH_AUTH_SOCK:-}"

for benchmark_user in runner grader sitecmd builder; do
  test "$(stat -c %a "/home/$benchmark_user")" = 700
  if runuser -u "$benchmark_user" -- sudo -n true 2>/dev/null; then
    echo "$benchmark_user unexpectedly has administrator privileges" >&2
    exit 1
  fi
done
runuser -u runner -- test ! -r /srv/sitecmd-benchmark/graders/canary
runuser -u runner -- test ! -r /srv/sitecmd-benchmark/app-data
systemctl is-active --quiet sitecmd-benchmark-firewall.service
nft list table inet sitecmd_benchmark >/dev/null
benchmark_denied_before=$(nft -j list counter inet sitecmd_benchmark private_egress_denied | jq '[.nftables[].counter.packets // empty] | add')
if runuser -u runner -- curl --noproxy '*' --silent --connect-timeout 3 --max-time 5 --output /dev/null https://192.168.5.2; then
  echo "The guest reached a private network address" >&2
  exit 1
fi
benchmark_denied_after=$(nft -j list counter inet sitecmd_benchmark private_egress_denied | jq '[.nftables[].counter.packets // empty] | add')
test "$benchmark_denied_after" -gt "$benchmark_denied_before"

benchmark_lock=/opt/sitecmd-benchmark/runtime-lock.json
test "$(runuser -u runner -- node --version)" = "v$(jq -r '.node.version' "$benchmark_lock")"
test "$(runuser -u runner -- pnpm --version)" = "$(jq -r '.pnpm' "$benchmark_lock")"
test "$(runuser -u builder -- rustc --version | cut -d ' ' -f 2)" = "$(jq -r '.rust' "$benchmark_lock")"
test "$(runuser -u builder -- cargo --version | cut -d ' ' -f 2)" = "$(jq -r '.rust' "$benchmark_lock")"
runuser -u runner -- curl --noproxy '*' --fail --silent --show-error --max-time 15 --output /dev/null \
  "https://nodejs.org/dist/v$(jq -r '.node.version' "$benchmark_lock")/SHASUMS256.txt"
pkg-config --exists webkit2gtk-4.1 gtk+-3.0
runuser -u sitecmd -- env LIBGL_ALWAYS_SOFTWARE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  xvfb-run -a dbus-run-session -- python3 /opt/sitecmd-benchmark/verify-webkit.py
echo "VM verification passed: native Linux, no host mounts, separated users, firewall, pinned toolchains, WebKit rendering"
