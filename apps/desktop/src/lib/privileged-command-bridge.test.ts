import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";
import {
  NATIVE_INTENT_CONNECTOR_COMMANDS,
  NATIVE_INTENT_FILESYSTEM_COMMANDS,
  PRIVILEGED_TOKEN_EXPIRED_MARKER,
  isPrivilegedTokenExpiredError,
  resolveCommandTimeoutMs,
  usesNativeResponseEvent,
} from "./privileged-command-bridge";

// Locate Rust fixtures from the working directory and fail if they are absent.
function srcTauriFile(...segments: string[]): string {
  const relative = path.join("src-tauri", ...segments);
  for (let dir = process.cwd(); ; dir = path.dirname(dir)) {
    const candidate = path.join(dir, relative);
    if (existsSync(candidate)) return candidate;
    if (path.dirname(dir) === dir) {
      throw new Error(`could not locate ${relative} from ${process.cwd()}`);
    }
  }
}

/** The Rust side's registry of commands that show a native confirmation dialog. */
function commandSecurityPath(): string {
  return srcTauriFile("permissions", "command-security.json");
}

function nativeIntentManifest(): Record<string, string[]> {
  const registry = JSON.parse(readFileSync(commandSecurityPath(), "utf8")) as {
    nativeIntentBrokerCommands: Record<string, string[]>;
  };
  return registry.nativeIntentBrokerCommands;
}

describe("resolveCommandTimeoutMs", () => {
  it("falls back to the 15s default for unlisted commands", () => {
    expect(resolveCommandTimeoutMs("get_integrations")).toBe(15_000);
  });

  it("gives live external-API fetches headroom beyond the default", () => {
    for (const command of [
      "fetch_integration_data",
      "fetch_analytics",
      "fetch_github_data",
      "get_pagespeed_report",
    ]) {
      expect(resolveCommandTimeoutMs(command)).toBeGreaterThan(15_000);
    }
  });

  it("gives the dependency sweep headroom for its per-package registry waves", () => {
    // The sweep performs several bounded registry waves, not one round trip.
    expect(resolveCommandTimeoutMs("detect_updates")).toBeGreaterThanOrEqual(90_000);
  });

  it("outlasts the native HTTP bound on a license validation", () => {
    // The bridge must outlast the native network bound.
    const constants = readFileSync(srcTauriFile("src", "constants.rs"), "utf8");
    const bound = /HTTP_CLIENT_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(
      constants,
    );
    if (!bound) throw new Error("HTTP_CLIENT_TIMEOUT not found in constants.rs");
    expect(resolveCommandTimeoutMs("validate_license")).toBeGreaterThan(Number(bound[1]) * 1000);
    // Allow two validation passes, their DB work, and a keychain prompt.
    const db = /DB_OP_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    if (!db) throw new Error("DB_OP_TIMEOUT not found in constants.rs");
    expect(resolveCommandTimeoutMs("validate_license")).toBeGreaterThanOrEqual(
      2 * ((Number(db[1]) * 3 + Number(bound[1])) * 1000 + 3 * 60_000),
    );
  });

  it("outlasts the whole git chain on a repository status read", () => {
    // Budget for the database lookup followed by five sequential git commands.
    const git = readFileSync(srcTauriFile("src", "core", "git.rs"), "utf8");
    const bound = /GIT_COMMAND_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(git);
    if (!bound) throw new Error("GIT_COMMAND_TIMEOUT not found in core/git.rs");
    const constants = readFileSync(srcTauriFile("src", "constants.rs"), "utf8");
    const db = /DB_OP_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    if (!db) throw new Error("DB_OP_TIMEOUT not found in constants.rs");
    expect(resolveCommandTimeoutMs("get_git_status")).toBeGreaterThan(
      (Number(bound[1]) * 5 + Number(db[1])) * 1000,
    );
  });

  it("outlasts the label-and-create chain on a tracker ticket", () => {
    // Budget for four API calls, ten DB dispatches, and a keychain prompt.
    const constants = readFileSync(srcTauriFile("src", "constants.rs"), "utf8");
    const api = /API_TIMEOUT_SHORT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    const db = /DB_OP_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    if (!api || !db) throw new Error("API_TIMEOUT_SHORT / DB_OP_TIMEOUT not found");
    expect(resolveCommandTimeoutMs("create_issue_link")).toBeGreaterThan(
      (Number(api[1]) * 4 + Number(db[1]) * 10) * 1000 + 3 * 60_000,
    );
  });

  it("outlasts the device-code request on a GitHub connect", () => {
    // Include the tier-gate DB read before the device-code request.
    const constants = readFileSync(srcTauriFile("src", "constants.rs"), "utf8");
    const bound = /\bAPI_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    const db = /DB_OP_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)/.exec(constants);
    if (!bound || !db) throw new Error("API_TIMEOUT / DB_OP_TIMEOUT not found in constants.rs");
    expect(resolveCommandTimeoutMs("connect_github")).toBeGreaterThan(
      (Number(bound[1]) + Number(db[1])) * 1000,
    );
  });

  it("gives an update download an unbounded-stream ceiling, not a request deadline", () => {
    expect(resolveCommandTimeoutMs("download_and_install_app_update")).toBeGreaterThanOrEqual(
      10 * 60_000,
    );
  });

  it("gives every native-confirmed command a human-scale timeout", () => {
    // Read the native-confirmed registry so new dialog commands join this check.
    const registry = JSON.parse(readFileSync(commandSecurityPath(), "utf8")) as {
      nativeConfirmedCommands: Array<{ command: string }>;
    };
    expect(registry.nativeConfirmedCommands.length).toBeGreaterThan(0);
    const onTheDefault = registry.nativeConfirmedCommands
      .map((entry) => entry.command)
      .filter((command) => resolveCommandTimeoutMs(command) <= 15_000);
    expect(onTheDefault).toEqual([]);
  });
});

describe("native user-intent commands", () => {
  it("keeps browser links brokered without a system confirmation", () => {
    const registry = JSON.parse(readFileSync(commandSecurityPath(), "utf8")) as {
      elevatedCommands: string[];
      nativeConfirmedCommands: Array<{ command: string }>;
    };
    expect(registry.elevatedCommands).toContain("open_external_url");
    expect(registry.nativeConfirmedCommands.map(({ command }) => command)).not.toContain(
      "open_external_url",
    );
    expect(NATIVE_INTENT_CONNECTOR_COMMANDS.has("open_external_url")).toBe(false);
    expect(nativeIntentManifest().run_external_connector_command).not.toContain(
      "open_external_url",
    );
    expect(resolveCommandTimeoutMs("open_external_url")).toBe(15_000);
  });

  it("does not put a system dialog in front of the coding agent handoff", () => {
    // The handoff opens the agent's app with a prompt staged in its
    // composer; nothing runs until the person sends it there.
    expect(NATIVE_INTENT_FILESYSTEM_COMMANDS.has("launch_agent_handoff")).toBe(false);
    expect(nativeIntentManifest().run_filesystem_access_command).not.toContain(
      "launch_agent_handoff",
    );
  });

  it("matches the audited command-security manifest", () => {
    const manifest = nativeIntentManifest();
    expect([...NATIVE_INTENT_CONNECTOR_COMMANDS].sort()).toEqual(
      [...manifest.run_external_connector_command].sort(),
    );
    expect([...NATIVE_INTENT_FILESYSTEM_COMMANDS].sort()).toEqual(
      [...manifest.run_filesystem_access_command].sort(),
    );
  });
});

describe("native bridge responses", () => {
  it("returns dependency sweeps directly to the main window", () => {
    expect(usesNativeResponseEvent("detect_updates")).toBe(true);
  });
});

describe("privileged bridge bootstrap", () => {
  it("does not initialize main-window telemetry inside broker windows", () => {
    const main = readFileSync(path.join(path.dirname(srcTauriFile()), "src", "main.tsx"), "utf8");
    expect(main).toContain("const privilegedBridgeWindow = isPrivilegedBridgeWindow();");
    expect(main).toContain("if (!privilegedBridgeWindow) initializeTelemetryFromStoredConsent();");
    expect(main).toContain("if (privilegedBridgeWindow) {");
  });
});

describe("isPrivilegedTokenExpiredError", () => {
  it("matches the exact marker string the Rust broker returns", () => {
    expect(isPrivilegedTokenExpiredError(new Error(PRIVILEGED_TOKEN_EXPIRED_MARKER))).toBe(true);
  });

  it("matches when the marker is embedded in a longer error message", () => {
    expect(
      isPrivilegedTokenExpiredError(
        new Error(`broker rejected request: ${PRIVILEGED_TOKEN_EXPIRED_MARKER}`),
      ),
    ).toBe(true);
  });

  it("matches a plain string carrying the marker (not an Error instance)", () => {
    expect(isPrivilegedTokenExpiredError(PRIVILEGED_TOKEN_EXPIRED_MARKER)).toBe(true);
  });

  it("returns false for unrelated errors", () => {
    expect(isPrivilegedTokenExpiredError(new Error("Network error"))).toBe(false);
    expect(isPrivilegedTokenExpiredError(new Error("token not found"))).toBe(false);
    expect(isPrivilegedTokenExpiredError("permission denied")).toBe(false);
  });

  it("returns false for null/undefined/empty", () => {
    expect(isPrivilegedTokenExpiredError(null)).toBe(false);
    expect(isPrivilegedTokenExpiredError(undefined)).toBe(false);
    expect(isPrivilegedTokenExpiredError("")).toBe(false);
  });
});

describe("late resolutions", () => {
  function bridgeSource(): string {
    const relative = path.join("src", "lib", "privileged-command-bridge.ts");
    for (let dir = process.cwd(); ; dir = path.dirname(dir)) {
      const candidate = path.join(dir, relative);
      if (existsSync(candidate)) return readFileSync(candidate, "utf8");
      if (path.dirname(dir) === dir) {
        throw new Error(`could not locate ${relative} from ${process.cwd()}`);
      }
    }
  }

  it("keeps the response listener alive past the budget and delivers the late outcome", () => {
    const source = bridgeSource();
    const rejectAt = source.indexOf("rejectResponse(createPrivilegedCommandTimeoutError");
    expect(rejectAt).toBeGreaterThan(-1);
    const timerStart = source.lastIndexOf("timer = window.setTimeout(() => {", rejectAt);
    expect(timerStart).toBeGreaterThan(-1);
    const timerEnd = source.indexOf("}, responseTimeoutMs);", timerStart);
    expect(timerEnd).toBeGreaterThan(timerStart);
    const timerBody = source.slice(timerStart, timerEnd);
    expect(timerBody).toContain("timedOut = true;");
    expect(timerBody).not.toContain("cleanup()");
    // The listener's already-settled branch must deliver the late payload
    // rather than dropping it.
    expect(source).toContain("if (timedOut) {");
    expect(source).toContain("deliverLateResolution({");
  });

  it("delivers a late outcome only for the newest invocation of its command", () => {
    // A late result must not overwrite a newer invocation's verdict.
    const source = bridgeSource();
    const register = source.indexOf("latestInvocationSeq.set(command, invocationSeq);");
    expect(register).toBeGreaterThan(-1);
    const lateBranch = source.indexOf("if (timedOut) {");
    expect(lateBranch).toBeGreaterThan(-1);
    const deliver = source.indexOf("deliverLateResolution({", lateBranch);
    expect(deliver).toBeGreaterThan(lateBranch);
    const gate = source.indexOf(
      "if (latestInvocationSeq.get(command) !== invocationSeq) return;",
      lateBranch,
    );
    expect(gate).toBeGreaterThan(lateBranch);
    expect(gate).toBeLessThan(deliver);
  });
});
