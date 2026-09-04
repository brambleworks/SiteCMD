# Disposable benchmark VM

This environment keeps benchmark builds and agent-generated patches off the
maintainer's host. It provisions Linux build dependencies and verifies WebKit can
load a page under a virtual display. It does not build or launch SiteCMD, install
agent clients, authenticate accounts, or run a benchmark trial.

## Run it

Requires Apple Silicon macOS, the repository's Node and pnpm versions, internet
access, and at least 45 GiB of free storage for initial setup. The VM uses four
CPUs, 6 GiB of RAM while running, and a 32 GiB sparse disk. Setup downloads a pinned
Lima runtime and Ubuntu image; it does not require Docker or change Homebrew.

Run from the repository root:

```bash
pnpm benchmark:vm setup
pnpm benchmark:vm status
pnpm benchmark:vm verify
pnpm benchmark:vm shell
pnpm benchmark:vm stop
```

`setup` starts the VM and verifies it. `shell` opens an unprivileged `runner`
session; use `exit` to return to the host. `pnpm benchmark:vm start` starts and
verifies an existing VM; `verify` can recheck it at any time. Verification requires public internet
access but does not contact model providers or spend account allowance. Keep the
VM stopped when not in use. It is not configured to start at host login.
`stop` requests a clean guest shutdown before stopping Lima's host controller; it
does not force-kill a running guest or discard its disk.

VM state, its generated management key, and the runtime stay under the ignored
`tools/benchmark/.work/` directory. Lima also caches the public OS image under
macOS's `~/Library/Caches/lima`. Cache and runtime storage are additional to the
sparse guest disk. No trial artifacts or credentials belong in Git.

## Isolation boundaries

[Lima plain mode](https://lima-vm.io/docs/config/plain/) uses Apple's native
virtualization without host folder mounts, SSH-agent forwarding, guest-agent
integration, or automatic application port forwarding. The only host listener is
Lima's loopback SSH management port, reported by `status`. Host API keys, proxy
variables, existing SSH keys, and agent configurations are not forwarded.

Guest roles have private homes and no administrator privileges:

| User      | Workspace                           | Purpose                                         |
| --------- | ----------------------------------- | ----------------------------------------------- |
| `runner`  | `/srv/sitecmd-benchmark/workspaces` | Agent workspaces and future subscription logins |
| `grader`  | `/srv/sitecmd-benchmark/graders`    | Independent acceptance tests and receipts       |
| `sitecmd` | `/srv/sitecmd-benchmark/app-data`   | Isolated desktop data                           |
| `builder` | `/srv/sitecmd-benchmark/build`      | Product builds                                  |

The separate `benchadmin` account is reserved for host-controlled provisioning.
Never run agents under that account or give them its management key. Guest user
separation protects files, not against a guest kernel exploit. A VM is not a
guarantee against all malicious code.

The guest firewall rejects non-loopback private/reserved IPv4 destinations and
external IPv6. Public HTTP/HTTPS, DNS, and NTP are allowed; the guest's DNS resolver
has a narrow private-address exception. Verification checks an actual rejected
connection against the firewall counter and confirms public dependency access.
This is not a public-domain allowlist: malicious code could still exfiltrate
anything it can read in the guest. Only approved benchmark cases and dedicated
guest credentials should enter it.

The build environment includes GTK/WebKit, Node, pnpm, and Rust. GUI verification
uses software rendering in Xvfb, with WebKit's sandbox retained. This verifies a
Linux rendering prerequisite, not SiteCMD's complete desktop or MCP workflow, and
does not establish performance parity with macOS or a hardware-accelerated desktop.

## Frozen configuration

`runtime-lock.json` pins the OS image and toolchain releases; downloaded Lima,
Node, and rustup artifacts are checked against their SHA-256 digests. Guest package
versions are captured in `/opt/sitecmd-benchmark/installed-packages.txt`.
Distribution packages are resolved at initial provisioning, not archive-pinned.
Automatic guest package upgrades are disabled to avoid changes mid-study. Rebuild
the environment with reviewed updates before a new study; do not use this VM as a
general-purpose or long-lived server.

Source provisioning files and the generated instance configuration are hashed at
creation. `start`, `verify`, and `shell` reject changes. A provisioning failure is
an error even if Lima reports that SSH is ready. Do not edit the hash receipt to
silence an error or make manual changes to a VM used for measured trials.

To replace a stale VM, first stop it and preserve any evidence. Use the bundled
Lima CLI with `LIMA_HOME` set to this repository's absolute `.work/lima` path to
rename the exact stopped `sitecmd-bench` instance to a short archival name. Run
`setup` to create a new instance. There is no automatic delete/reset command;
never remove a broad VM state directory to resolve a setup error.

## Before real trials

The [pilot protocol](../../../docs/qa/agent-workflow-benchmark.md) still requires
validated cases, an agent/desktop execution adapter, and an independent grader.
Install and authenticate the selected clients inside the VM using subscription
login, never by copying host credential files. Verify exact model access, disabled
paid overage/fallback, and fresh account quota evidence before any prompts.

The desktop must own its own guest database; `SITECMD_DB_PATH` does not redirect
the desktop. The future adapter must export only an approved source snapshot,
protect grader files, capture complete usage, and enforce the pilot's stop limits.
VM setup alone does not authorize API charges, paid resets, or trial execution.
