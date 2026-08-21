const ACTION = ".github/actions/sitecmd-gate/action.yml";
const MINT_CARD = "apps/desktop/src/components/settings/ConnectedServiceManagement.tsx";

// Claims every CI credential surface must include.
const CAPABILITIES = [
  { name: "the gate", pattern: /gate/i },
  { name: "the deployment-ordering cursor", pattern: /deployment-ordering cursor/i },
  {
    name: "deployment notification",
    pattern: /\b(notify|notifies|record|records)\s+deployments\b/i,
  },
  { name: "code evidence submission", pattern: /(code )?evidence/i },
  { name: "the findings read boundary", pattern: /cannot read (the site's )?findings/i },
];

/** The composite action's ci-token input description, as one string. */
function ciTokenDescription(source) {
  const input = source.split(/^ {2}ci-token:$/m)[1];
  if (!input) return null;
  // The description block ends where the next input key starts.
  return input.split(/^ {2}\w[\w-]*:$/m)[0] ?? input;
}

export function ciCredentialDisclosureFailures(read) {
  const failures = [];

  const action = read(ACTION);
  const description = ciTokenDescription(action);
  if (!description) {
    failures.push(`${ACTION} no longer declares the ci-token input these rules pin`);
  } else {
    for (const capability of CAPABILITIES) {
      if (!capability.pattern.test(description)) {
        failures.push(
          `${ACTION}: the ci-token description no longer states ${capability.name}; ` +
            `the credential's description must name everything it can do`,
        );
      }
    }
  }

  const mintCard = read(MINT_CARD);
  for (const capability of CAPABILITIES) {
    if (!capability.pattern.test(mintCard)) {
      failures.push(
        `${MINT_CARD}: the CI credential card no longer states ${capability.name}; ` +
          `the mint copy must name everything the token can do`,
      );
    }
  }

  return failures;
}
