const CONSTANTS = "apps/desktop/src-tauri/src/constants.rs";
const BRIDGE = "apps/desktop/src/lib/privileged-command-bridge.ts";

/** Seconds in `Duration::from_secs(N)` for the named Rust constant. */
function rustTimeoutSeconds(source, name) {
  const match = new RegExp(
    `${name}\\s*:\\s*Duration\\s*=\\s*Duration::from_secs\\(([\\d_]+)\\)`,
  ).exec(source);
  return match ? Number(match[1].replaceAll("_", "")) : null;
}

/** Milliseconds for a `const NAME = <number>;` or `const NAME = <a> * <b>;`. */
function tsTimeoutMs(source, name) {
  const match = new RegExp(`const\\s+${name}\\s*=\\s*([\\d_]+)(?:\\s*\\*\\s*([\\d_]+))?\\s*;`).exec(
    source,
  );
  if (!match) return null;
  const left = Number(match[1].replaceAll("_", ""));
  return match[2] ? left * Number(match[2].replaceAll("_", "")) : left;
}

export function confirmDeadlineFailures(read) {
  const failures = [];
  const nativeSeconds = rustTimeoutSeconds(read(CONSTANTS), "SENSITIVE_CONFIRM_TIMEOUT");
  const bridgeMs = tsTimeoutMs(read(BRIDGE), "HUMAN_CONFIRMATION_TIMEOUT_MS");
  if (nativeSeconds === null) {
    failures.push(`Could not read SENSITIVE_CONFIRM_TIMEOUT from ${CONSTANTS}.`);
  }
  if (bridgeMs === null) {
    failures.push(`Could not read HUMAN_CONFIRMATION_TIMEOUT_MS from ${BRIDGE}.`);
  }
  if (nativeSeconds === null || bridgeMs === null) return failures;
  if (nativeSeconds * 1000 >= bridgeMs) {
    failures.push(
      `SENSITIVE_CONFIRM_TIMEOUT (${nativeSeconds}s, ${CONSTANTS}) must expire BEFORE HUMAN_CONFIRMATION_TIMEOUT_MS (${bridgeMs / 1000}s, ${BRIDGE}). Otherwise the bridge gives up first and reports a client-side timeout while the native dialog is still waiting for an answer, and the user is told the wrong thing about a destructive action.`,
    );
  }
  return failures;
}
