import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const smoke = readFileSync("docs/operations/launch-smoke.md", "utf8");

describe("launch smoke coverage", () => {
  it("covers every public release artifact and the local product boundary", () => {
    expect(smoke).toContain("## 2. Desktop core flow");
    expect(smoke).toContain("sitecmd scan --url");
    expect(smoke).toContain("sitecmd audit . --format summary");
    expect(smoke).toContain("## 4. Bundled MCP server");
    expect(smoke).toContain("`get_projects`, `get_issues`, and");
    expect(smoke).toContain("`request_verification`");
    expect(smoke).toContain("## 6. Update loop");
    expect(smoke).toContain(
      "complete local product remains available without a connected entitlement",
    );
  });

  it("keeps private service operations and point-in-time results out", () => {
    for (const privateDetail of [
      "wrangler",
      "D1",
      "ATTEMPT_KEYRING",
      "site_credentials",
      "migration 0038",
      "id-token: write",
      "Last automated production pass",
      "Last full human pass",
      "Founder acceptance",
    ]) {
      expect(smoke).not.toContain(privateDetail);
    }
    expect(smoke).toContain("Keep the completed record");
    expect(smoke).toContain("private connected-service");
  });
});
