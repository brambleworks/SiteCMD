import { spawnSync } from "node:child_process";
import { mkdirSync, chownSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";

export function systemCommand(command, args) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    timeout: 30000,
    maxBuffer: 1024 * 1024,
  });
  if (result.status !== 0)
    throw new Error(`${command}: ${result.stderr || result.error?.message || result.status}`);
  return result.stdout.trim();
}

export async function startDesktop(id, binary) {
  if (!/^[a-f0-9]{24,64}$/.test(id)) throw new Error("Invalid desktop session identity");
  const data = `/srv/sitecmd-benchmark/app-data/${id}`;
  const uid = Number(systemCommand("id", ["-u", "sitecmd"]));
  const gid = Number(systemCommand("id", ["-g", "sitecmd"]));
  mkdirSync(data, { mode: 0o700 });
  chownSync(data, uid, gid);
  const unit = `sitecmd-desktop-${id}`;
  systemCommand("systemd-run", [
    `--unit=${unit}`,
    "--collect",
    "--property=User=sitecmd",
    "--property=MemoryMax=2G",
    "--property=TasksMax=256",
    "--property=RuntimeMaxSec=1800",
    "--setenv=LIBGL_ALWAYS_SOFTWARE=1",
    "--setenv=WEBKIT_DISABLE_DMABUF_RENDERER=1",
    `--setenv=XDG_DATA_HOME=${data}`,
    "xvfb-run",
    "-a",
    "dbus-run-session",
    "--",
    "tauri-driver",
    "--port",
    "4444",
    "--native-port",
    "4445",
  ]);
  const request = async (route, body, method = "POST") => {
    const response = await fetch(`http://127.0.0.1:4444${route}`, {
      method,
      headers: { "Content-Type": "application/json" },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      signal: AbortSignal.timeout(120000),
    });
    const result = await response.json();
    if (!response.ok || result.value?.error)
      throw new Error(`Desktop WebDriver: ${JSON.stringify(result.value)}`);
    return result.value;
  };
  try {
    let started = false;
    for (let index = 0; index < 60; index++) {
      try {
        await request("/status", undefined, "GET");
        started = true;
        break;
      } catch {
        await delay(500);
      }
    }
    if (!started) throw new Error("Desktop driver did not start");
    const created = await request("/session", {
      capabilities: { alwaysMatch: { "tauri:options": { application: binary } } },
    });
    const session = created.sessionId;
    await request(`/session/${session}/timeouts`, { script: 120000 });
    const invoke = async (command, args = {}) => {
      const result = await request(`/session/${session}/execute/async`, {
        script:
          "const [command, args, done] = arguments; window.__TAURI_INTERNALS__.invoke(command, args).then(value => done({value}), error => done({error: String(error)}));",
        args: [command, args],
      });
      if (result.error) throw new Error(`${command}: ${result.error}`);
      return result.value;
    };
    if ((await invoke("ping")) !== "pong") throw new Error("Desktop health check failed");
    return {
      invoke,
      data,
      unit,
      database: `${data}/com.sitecmd.app/sitecmd.db`,
      close: () => systemCommand("systemctl", ["stop", unit]),
    };
  } catch (error) {
    systemCommand("systemctl", ["stop", unit]);
    throw error;
  }
}
