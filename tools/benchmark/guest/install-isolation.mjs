import { readFileSync, writeFileSync } from "node:fs";
import { systemCommand } from "./desktop-session.mjs";

if (process.platform !== "linux" || process.getuid() !== 0)
  throw new Error("Guest controller required");
const profile = readFileSync(new URL("./bwrap.apparmor", import.meta.url), "utf8");
const target = "/etc/apparmor.d/sitecmd-benchmark-bwrap";
// "wx" fails when the profile is already there, so let the write decide and
// compare only what already exists. Checking first raced whoever wrote it.
try {
  writeFileSync(target, profile, { flag: "wx", mode: 0o644 });
} catch (error) {
  if (error.code !== "EEXIST") throw error;
  if (readFileSync(target, "utf8") !== profile)
    throw new Error("Existing sandbox profile differs; review before replacing it", {
      cause: error,
    });
}
systemCommand("apparmor_parser", ["-r", target]);
systemCommand("sudo", [
  "-u",
  "runner",
  "bwrap",
  "--unshare-user",
  "--unshare-net",
  "--ro-bind",
  "/usr",
  "/usr",
  "--symlink",
  "usr/lib",
  "/lib",
  "--",
  "/usr/bin/true",
]);
const runner = Number(systemCommand("id", ["-u", "runner"]));
const rules = `add table inet sitecmd_benchmark_control\nflush table inet sitecmd_benchmark_control\ntable inet sitecmd_benchmark_control {\n chain output {\n type filter hook output priority -10; policy accept;\n meta skuid ${runner} tcp dport { 4444, 4445 } reject\n }\n}\n`;
const firewall = "/etc/sitecmd-benchmark-control.nft";
let firewallInstalled = false;
try {
  writeFileSync(firewall, rules, { flag: "wx", mode: 0o600 });
  firewallInstalled = true;
} catch (error) {
  if (error.code !== "EEXIST") throw error;
  if (readFileSync(firewall, "utf8") !== rules)
    throw new Error("Control firewall changed", { cause: error });
}
if (firewallInstalled) systemCommand("nft", ["-f", firewall]);
const service = `[Unit]\nDescription=SiteCMD benchmark control port isolation\nAfter=sitecmd-benchmark-firewall.service\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=/usr/sbin/nft -f /etc/sitecmd-benchmark-control.nft\n[Install]\nWantedBy=multi-user.target\n`;
const servicePath = "/etc/systemd/system/sitecmd-benchmark-control.service";
try {
  writeFileSync(servicePath, service, { flag: "wx", mode: 0o644 });
} catch (error) {
  if (error.code !== "EEXIST") throw error;
}
systemCommand("systemctl", ["daemon-reload"]);
systemCommand("systemctl", ["enable", "sitecmd-benchmark-control.service"]);
console.log("Sandbox launcher and private desktop control ports configured");
