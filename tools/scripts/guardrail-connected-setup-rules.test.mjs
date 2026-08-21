import { describe, expect, it } from "vitest";

import { connectedSetupFailures } from "./lib/guardrail-connected-setup-rules.mjs";

const SETUP = "apps/desktop/src-tauri/src/commands/connected_setup.rs";
const CLIENT = "apps/desktop/src-tauri/src/connected_service.rs";

const SETUP_BASE = [
  "pub async fn create_connected_site(",
  ") -> Result<ConnectedSiteChallenge, String> {",
  "    if installation_token.trim().is_empty() {",
  '        return Err("an installation token is required".into());',
  "    }",
  "    let created = client.create_site(url.trim(), None).await?;",
  "    if let Err(error) = connect_locally() {",
  "        let _ = client.delete_site(&created.id).await;",
  "        return Err(error);",
  "    }",
  "    if let Err(error) = store_token() {",
  "        let _ = client.delete_site(&created.id).await;",
  "        return Err(restore(error));",
  "    }",
  "    Ok(challenge)",
  "}",
  "",
  "pub async fn fetch_connected_site_state(",
].join("\n");

const CLIENT_BASE = "impl Client {\n    pub async fn delete_site(&self) {}\n}";

function run(overrides = {}) {
  const fixture = { [SETUP]: SETUP_BASE, [CLIENT]: CLIENT_BASE, ...overrides };
  return connectedSetupFailures((file) => fixture[file] ?? "");
}

describe("connectedSetupFailures", () => {
  it("accepts a create path whose failures all clean up the remote site", () => {
    expect(run()).toEqual([]);
  });

  it("ignores failure returns before the remote create", () => {
    expect(run()).toEqual([]);
  });

  it("rejects a failure return with no cleanup above it", () => {
    const failures = run({
      [SETUP]: SETUP_BASE.replace(
        "        let _ = client.delete_site(&created.id).await;\n        return Err(restore(error));",
        "        return Err(restore(error));",
      ),
    });
    expect(failures.some((failure) => failure.includes("does not delete the remote site"))).toBe(
      true,
    );
  });

  it("rejects a client that loses the delete_site method", () => {
    const failures = run({ [CLIENT]: "impl Client {}" });
    expect(failures.some((failure) => failure.includes("no longer offers delete_site"))).toBe(true);
  });

  it("flags the rules as stale when the create call disappears", () => {
    const failures = run({
      [SETUP]: SETUP_BASE.replace(".create_site(", ".open_site("),
    });
    expect(failures.some((failure) => failure.includes("update these rules"))).toBe(true);
  });
});
