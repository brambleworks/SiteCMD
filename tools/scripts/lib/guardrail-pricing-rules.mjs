const MODEL = "apps/desktop/src/lib/commercial-model.json";
const ACCT = "apps/desktop/src/components/settings/AccountSettings.tsx";

const EXPECTED_MODEL = {
  billableUnit: "connected_production_site",
  connectedServiceAccess: "comped_beta",
  localWorkbench: "free_complete",
  meteredOverages: false,
  paidBoundary: "connected_service",
  planShape: "flat_bundles",
  publicPricing: "not_set",
};

export function pricingConsistencyFailures(read, exists) {
  const failures = [];

  if (!exists(MODEL)) {
    failures.push(`${MODEL} is missing; the connected-service commercial model must be explicit.`);
  } else {
    try {
      const model = JSON.parse(read(MODEL));
      if (JSON.stringify(model) !== JSON.stringify(EXPECTED_MODEL)) {
        failures.push(
          `${MODEL} must match the connected-service commercial model: complete free local workbench, comped connected service, no public price, flat bundles, and no metered overages.`,
        );
      }
    } catch (error) {
      failures.push(`${MODEL} is not valid JSON: ${error.message}`);
    }
  }

  if (!exists(ACCT)) return failures;
  const account = read(ACCT);

  if (
    /\$\d+|\/mo\b|\/yr\b|\bcheckout\b|useCheckout|getCheckoutUrl|@\/lib\/pricing|Get Plus|Get Professional/i.test(
      account,
    )
  ) {
    failures.push(`${ACCT} must not expose a price or checkout before the public pricing pass.`);
  }

  if (
    /\b3 scans (?:\/|per) day\b|\b3 verified agent fixes\b|\b10 recent runs\b|\bup to (?:3|10) (?:production )?sites\b|summary-level|full-depth MCP|starting point for each fix/i.test(
      account,
    )
  ) {
    failures.push(`${ACCT} reintroduces a retired local-workbench information or usage gate.`);
  }

  for (const [label, pattern] of [
    ["the complete free desktop workbench", /desktop workbench is free and complete/i],
    ["free connected beta access", /free during the beta/i],
    ["the beta request action", /Request beta access/],
    ["the SiteCMD Connect name", /SiteCMD Connect/],
    ["existing-subscriber billing management", /Manage Billing/],
    ["the beta contact path", /https:\/\/sitecmd\.com\/contact/],
  ]) {
    if (!pattern.test(account)) failures.push(`${ACCT} must retain ${label}.`);
  }

  return failures;
}
