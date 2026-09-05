import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

export function openMcp(entry, database, log = () => {}, { user = "sitecmd", args = [] } = {}) {
  const child = spawn(
    "sudo",
    ["-u", user, "env", `SITECMD_DB_PATH=${database}`, "node", entry, ...args],
    {
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  const pending = new Map();
  let sequence = 0;
  let failure;
  const lines = createInterface({ input: child.stdout });
  child.stderr.on("data", (data) => log({ channel: "stderr", text: data.toString() }));
  const fail = (error) => {
    failure = error;
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer);
      reject(error);
    }
    pending.clear();
  };
  lines.on("line", (line) => {
    try {
      const response = JSON.parse(line);
      log({ direction: "response", message: response });
      const item = pending.get(response.id);
      if (!item) return;
      clearTimeout(item.timer);
      pending.delete(response.id);
      if (response.error) item.reject(new Error(JSON.stringify(response.error)));
      else item.resolve(response.result);
    } catch (error) {
      fail(error);
    }
  });
  child.on("error", fail);
  child.on("exit", (code) => fail(new Error(`SiteCMD MCP exited: ${code}`)));
  const send = (method, params, id) => {
    if (failure) throw failure;
    const message = {
      jsonrpc: "2.0",
      ...(id === undefined ? {} : { id }),
      method,
      ...(params === undefined ? {} : { params }),
    };
    log({ direction: "request", message });
    child.stdin.write(`${JSON.stringify(message)}\n`);
  };
  const request = (method, params) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`MCP request timed out: ${method}`));
      }, 120000);
      pending.set(id, { resolve, reject, timer });
      try {
        send(method, params, id);
      } catch (error) {
        clearTimeout(timer);
        pending.delete(id);
        reject(error);
      }
    });
  return {
    request,
    notify: (method, params) => send(method, params),
    call: (name, args) => request("tools/call", { name, arguments: args }),
    close: () => {
      child.stdin.end();
      child.kill("SIGTERM");
    },
  };
}

export async function initializeMcp(session, agent = "codex") {
  const result = await session.request("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: agent, version: "benchmark-calibration-1" },
  });
  session.notify("notifications/initialized");
  return result;
}
