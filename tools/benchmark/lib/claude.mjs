import { spawnSync } from "node:child_process";
import path from "node:path";

const MINIMUM_CLAUDE_VERSION = [2, 1, 219];
const REQUIRED_CLAUDE_FLAGS = [
  "--disallowedTools",
  "--mcp-config",
  "--no-session-persistence",
  "--permission-mode",
  "--safe-mode",
  "--setting-sources",
  "--settings",
  "--strict-mcp-config",
  "--tools",
];
const SAFE_BASH_ENVIRONMENT = new Set([
  "HOME",
  "LANG",
  "LOGNAME",
  "PATH",
  "SHELL",
  "TERM",
  "TMPDIR",
  "USER",
]);
const ALLOWED_BASH_DOMAINS = ["registry.npmjs.org"];
let sandboxPreflightComplete = false;

export function parseClaudeVersion(output) {
  const match = /\b(\d+)\.(\d+)\.(\d+)\b/.exec(output);
  return match ? match.slice(1).map(Number) : null;
}

function compareVersions(left, right) {
  for (let index = 0; index < Math.max(left.length, right.length); index++) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

export function assertSandboxSupport(platform, versionOutput, helpOutput = null) {
  if (platform === "win32") {
    throw new Error(
      "the benchmark sandbox requires macOS, Linux, or WSL2; native Windows is unsupported",
    );
  }
  if (platform !== "darwin" && platform !== "linux") {
    throw new Error(`the benchmark sandbox does not support ${platform}`);
  }
  const version = parseClaudeVersion(versionOutput);
  if (!version || compareVersions(version, MINIMUM_CLAUDE_VERSION) < 0) {
    throw new Error(
      `Claude Code ${MINIMUM_CLAUDE_VERSION.join(".")} or newer is required for the fail-closed benchmark sandbox`,
    );
  }
  if (helpOutput != null) {
    const missing = REQUIRED_CLAUDE_FLAGS.filter((flag) => !helpOutput.includes(flag));
    if (missing.length > 0) {
      throw new Error(`the installed Claude Code lacks required flags: ${missing.join(", ")}`);
    }
  }
}

function protectedEnvironmentVariables(environment) {
  return Object.keys(environment)
    .filter(
      (name) =>
        /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) &&
        !SAFE_BASH_ENVIRONMENT.has(name) &&
        !name.startsWith("LC_"),
    )
    .sort()
    .map((name) => ({ name, mode: "deny" }));
}

export function buildClaudeInvocation({ cwd, model, maxTurns, environment = process.env }) {
  const workspace = path.resolve(cwd);
  const settings = {
    sandbox: {
      enabled: true,
      failIfUnavailable: true,
      autoAllowBashIfSandboxed: true,
      allowUnsandboxedCommands: false,
      filesystem: {
        denyRead: ["~/"],
        allowRead: [workspace],
      },
      credentials: {
        envVars: protectedEnvironmentVariables(environment),
      },
      network: {
        allowedDomains: ALLOWED_BASH_DOMAINS,
        strictAllowlist: true,
      },
    },
  };
  const args = [
    "-p",
    "--safe-mode",
    "--disable-slash-commands",
    "--no-chrome",
    "--no-session-persistence",
    "--output-format",
    "json",
    "--permission-mode",
    "dontAsk",
    "--tools",
    "Bash",
    "--disallowedTools",
    "mcp__*",
    "--setting-sources",
    "",
    "--strict-mcp-config",
    "--mcp-config",
    '{"mcpServers":{}}',
    "--settings",
    JSON.stringify(settings),
    "--model",
    model,
    "--max-turns",
    String(maxTurns),
  ];
  return {
    args,
    env: {
      ...environment,
      CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1",
      CLAUDE_CODE_SKIP_PROMPT_HISTORY: "1",
      CLAUDE_CODE_SUBPROCESS_ENV_SCRUB: "1",
      DISABLE_AUTOUPDATER: "1",
    },
  };
}

function verifySandboxSupport(spawn, platform, environment) {
  if (sandboxPreflightComplete) return;
  const result = spawn("claude", ["--version"], {
    encoding: "utf8",
    env: environment,
  });
  if (result.error) throw new Error(`claude version check failed: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`claude version check failed (exit ${result.status})`);
  }
  const help = spawn("claude", ["--help"], {
    encoding: "utf8",
    env: environment,
  });
  if (help.error) throw new Error(`claude capability check failed: ${help.error.message}`);
  if (help.status !== 0) {
    throw new Error(`claude capability check failed (exit ${help.status})`);
  }
  assertSandboxSupport(
    platform,
    result.stdout || result.stderr || "",
    help.stdout || help.stderr || "",
  );
  sandboxPreflightComplete = true;
}

export function runClaudeFix({
  prompt,
  cwd,
  model,
  maxTurns,
  timeoutMs = 20 * 60 * 1000,
  spawn = spawnSync,
  platform = process.platform,
  environment = process.env,
}) {
  const started = Date.now();
  try {
    verifySandboxSupport(spawn, platform, environment);
  } catch (error) {
    return { ok: false, error: error.message, wallMs: Date.now() - started };
  }
  const { args, env } = buildClaudeInvocation({ cwd, model, maxTurns, environment });
  const r = spawn("claude", args, {
    cwd,
    input: prompt,
    encoding: "utf8",
    env,
    timeout: timeoutMs,
    maxBuffer: 64 * 1024 * 1024,
  });
  const wallMs = Date.now() - started;

  if (r.error) {
    return { ok: false, error: `claude spawn failed: ${r.error.message}`, wallMs };
  }

  let parsed;
  try {
    parsed = JSON.parse(r.stdout);
  } catch {
    return {
      ok: false,
      error: `could not parse claude JSON (exit ${r.status})`,
      stderr: (r.stderr || "").slice(0, 2000),
      stdout: (r.stdout || "").slice(0, 2000),
      wallMs,
    };
  }

  const usage = parsed.usage || {};
  const inputTokens = usage.input_tokens || 0;
  const outputTokens = usage.output_tokens || 0;
  const cacheCreate = usage.cache_creation_input_tokens || 0;
  const cacheRead = usage.cache_read_input_tokens || 0;

  return {
    ok: r.status === 0 && !parsed.is_error,
    isError: Boolean(parsed.is_error),
    exitStatus: r.status,
    subtype: parsed.subtype,
    numTurns: parsed.num_turns || 0,
    durationMs: parsed.duration_ms || wallMs,
    wallMs,
    costUsd: parsed.total_cost_usd || 0,
    inputTokens,
    outputTokens,
    cacheCreate,
    cacheRead,
    totalTokens: inputTokens + outputTokens + cacheCreate + cacheRead,
    sessionId: parsed.session_id,
    resultText: (parsed.result || "").slice(0, 4000),
  };
}
