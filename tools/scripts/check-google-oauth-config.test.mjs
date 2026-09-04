import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const script = fileURLToPath(new URL("./check-google-oauth-config.mjs", import.meta.url));

function preflight(credentials = {}) {
  const env = { ...process.env };
  delete env.GOOGLE_CLIENT_ID;
  delete env.GOOGLE_CLIENT_SECRET;
  return spawnSync(process.execPath, [script], {
    env: { ...env, ...credentials },
    encoding: "utf8",
  });
}

describe("Google OAuth release preflight", () => {
  it("requires both credentials without relying on a local dotenv file", () => {
    const result = preflight();
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET");
    expect(result.stderr).toContain("release-signing");
  });

  it.each(["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"])("rejects missing or blank %s", (name) => {
    for (const value of [undefined, "", " \t\n"]) {
      const credentials = {
        GOOGLE_CLIENT_ID: "client-id-sentinel",
        GOOGLE_CLIENT_SECRET: "credential-sentinel",
      };
      if (value === undefined) delete credentials[name];
      else credentials[name] = value;
      const result = preflight(credentials);
      expect(result.status).toBe(1);
      expect(result.stderr).toContain(name);
      expect(result.stdout + result.stderr).not.toContain("sentinel");
    }
  });

  it("accepts complete credentials without printing their values", () => {
    const result = preflight({
      GOOGLE_CLIENT_ID: "client-id-sentinel",
      GOOGLE_CLIENT_SECRET: "credential-sentinel",
    });
    expect(result.status).toBe(0);
    expect(result.stdout + result.stderr).toBe("");
  });
});
