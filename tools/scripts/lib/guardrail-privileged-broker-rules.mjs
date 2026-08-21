const TOKEN_EXPIRED_MARKER = "Privileged command token is invalid or expired.";

export function privilegedBrokerTokenMarkerFailures(read) {
  const rust = read("apps/desktop/src-tauri/src/commands/privileged_command_broker/token_state.rs");
  const ts = read("apps/desktop/src/lib/privileged-command-bridge.ts");
  const failures = [];
  if (!rust.includes(`"${TOKEN_EXPIRED_MARKER}"`)) {
    failures.push(
      `token_state.rs must return the exact string "${TOKEN_EXPIRED_MARKER}" so the frontend retry helper can recognise it`,
    );
  }
  if (!ts.includes(`"${TOKEN_EXPIRED_MARKER}"`)) {
    failures.push(
      `privileged-command-bridge.ts must export PRIVILEGED_TOKEN_EXPIRED_MARKER as the exact Rust string for the auto-reissue retry to fire`,
    );
  }
  return failures;
}
