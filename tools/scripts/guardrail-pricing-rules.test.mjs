import { describe, expect, it } from "vitest";

import { pricingConsistencyFailures } from "./lib/guardrail-pricing-rules.mjs";

const MODEL = "apps/desktop/src/lib/commercial-model.json";
const ACCT = "apps/desktop/src/components/settings/AccountSettings.tsx";

const validModel = JSON.stringify({
  billableUnit: "connected_production_site",
  connectedServiceAccess: "comped_founder_beta",
  localWorkbench: "free_complete",
  meteredOverages: false,
  paidBoundary: "connected_service",
  planShape: "flat_bundles",
  publicPricing: "not_set",
});

const validAccount = [
  'const FOUNDER_BETA_CONTACT_URL = "https://sitecmd.com/contact";',
  'const copy = "The desktop workbench is free and complete";',
  'const beta = "Comped during the founder beta";',
  'const cta = "Request founder beta access";',
  'const legacy = "Manage Billing";',
].join("\n");

function failures(overrides = {}) {
  const tree = { [MODEL]: validModel, [ACCT]: validAccount, ...overrides };
  return pricingConsistencyFailures(
    (file) => tree[file],
    (file) => file in tree,
  );
}

describe("desktop commercial boundary", () => {
  it("accepts the complete free workbench and comped founder beta", () => {
    expect(failures()).toEqual([]);
  });

  it("rejects a speculative price or checkout flow", () => {
    expect(
      failures({ [ACCT]: `${validAccount}\nGet Plus for $29/mo via checkout` }),
    ).toContainEqual(expect.stringContaining("must not expose a price or checkout"));
  });

  it("rejects retired local limits", () => {
    expect(
      failures({ [ACCT]: `${validAccount}\n3 scans / day and 10 recent runs` }),
    ).toContainEqual(expect.stringContaining("retired local-workbench"));
  });

  it("requires the generated commercial model", () => {
    const bad = JSON.stringify({ ...JSON.parse(validModel), publicPricing: "29_monthly" });
    expect(failures({ [MODEL]: bad })).toContainEqual(
      expect.stringContaining("founder-beta commercial model"),
    );
  });
});
