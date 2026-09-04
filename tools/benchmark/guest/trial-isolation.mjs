import { systemCommand } from "./desktop-session.mjs";

export function verifyControlIsolation() {
  const uid = Number(systemCommand("id", ["-u", "runner"]));
  const rules = systemCommand("nft", ["-nn", "list", "table", "inet", "sitecmd_benchmark_control"]);
  if (!rules.includes(`meta skuid ${uid} tcp dport { 4444, 4445 } reject`))
    throw new Error("Private desktop control port firewall is not active");
  const probe = `
    const net = require('node:net');
    Promise.all([4444, 4445].map(port => new Promise((resolve, reject) => {
      const socket = net.connect({host:'127.0.0.1', port});
      socket.setTimeout(2000);
      socket.on('connect', () => {socket.destroy(); reject(new Error('Desktop control port is reachable'));});
      socket.on('timeout', () => {socket.destroy(); reject(new Error('Isolation probe timed out'));});
      socket.on('error', error => error.code === 'ECONNREFUSED' ? resolve() : reject(error));
    }))).catch(error => {console.error(error.message); process.exitCode = 1;});
  `;
  systemCommand("sudo", ["-u", "runner", "node", "-e", probe]);
}
