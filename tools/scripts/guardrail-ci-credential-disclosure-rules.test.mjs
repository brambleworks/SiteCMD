import { describe, expect, it } from "vitest";

import { ciCredentialDisclosureFailures } from "./lib/guardrail-ci-credential-disclosure-rules.mjs";

const ACTION = ".github/actions/sitecmd-gate/action.yml";
const MINT_CARD = "apps/desktop/src/components/settings/ConnectedServiceManagement.tsx";

const ACTION_BASE = [
  "inputs:",
  "  connection-export:",
  "    description: The encrypted connection export.",
  "  ci-token:",
  "    description: >-",
  "      The CI token minted for this site in the desktop app. Bound to one site,",
  "      it can read only the deployment-ordering cursor, run this gate, notify",
  "      deployments, and submit code evidence. It cannot read findings,",
  "      lifecycle state, or account data. This action uses it for the gate alone.",
  "    required: true",
  "  threshold:",
  "    description: The least severe NEW finding that fails the build.",
].join("\n");

const MINT_BASE = [
  "<p>A token for this site alone. It can ask whether a branch introduces findings the",
  "baseline does not have (the gate), read only the deployment-ordering cursor, record",
  "deployments, and submit code evidence from a checkout. It cannot read findings,",
  "lifecycle state, or account data.</p>",
].join("\n");

function run(overrides = {}) {
  const fixture = { [ACTION]: ACTION_BASE, [MINT_CARD]: MINT_BASE, ...overrides };
  return ciCredentialDisclosureFailures((file) => fixture[file] ?? "");
}

describe("ciCredentialDisclosureFailures", () => {
  it("accepts surfaces that name all four operations and the read boundary", () => {
    expect(run()).toEqual([]);
  });

  it("rejects an action description that drops deployment notification", () => {
    const failures = run({
      [ACTION]: ACTION_BASE.replace("notify", "mention"),
    });
    expect(failures.some((failure) => failure.includes("deployment notification"))).toBe(true);
  });

  it("rejects an action description that drops the evidence door", () => {
    const failures = run({
      [ACTION]: ACTION_BASE.replace("and submit code evidence.", ""),
    });
    expect(failures.some((failure) => failure.includes("code evidence submission"))).toBe(true);
  });

  it("rejects an action that no longer declares the ci-token input", () => {
    const failures = run({
      [ACTION]: ACTION_BASE.replace("  ci-token:", "  runner-token:"),
    });
    expect(failures.some((failure) => failure.includes("no longer declares"))).toBe(true);
  });

  it("rejects mint copy that drops the findings read boundary", () => {
    const failures = run({
      [MINT_CARD]: MINT_BASE.replace("It cannot read findings,", "It is limited,"),
    });
    expect(failures.some((failure) => failure.includes("findings read boundary"))).toBe(true);
  });

  it("rejects an action that drops the deployment cursor read", () => {
    const failures = run({
      [ACTION]: ACTION_BASE.replace("read only the deployment-ordering cursor, ", ""),
    });
    expect(failures.some((failure) => failure.includes("deployment-ordering cursor"))).toBe(true);
  });

  it("rejects mint copy that stops naming deployments", () => {
    const failures = run({
      [MINT_CARD]: MINT_BASE.replace("record", "list"),
    });
    expect(failures.some((failure) => failure.includes("deployment notification"))).toBe(true);
  });
});
