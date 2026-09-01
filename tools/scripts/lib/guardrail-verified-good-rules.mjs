const PROFILE = "apps/desktop/src-tauri/crates/engine/src/profile/mod.rs";
const PROJECTION = "apps/desktop/src-tauri/crates/engine/src/profile/projection.rs";
const SITE_FACTS = "apps/desktop/src-tauri/src/core/scanner/site_facts.rs";
const MIGRATION = "apps/desktop/src-tauri/src/db/migrations/018_verified_good_profile.sql";

// Return a public Rust function through its matching closing brace.
function functionBody(source, name) {
  const start = source.indexOf(`pub fn ${name}(`);
  if (start === -1) return null;
  const open = source.indexOf("{", start);
  if (open === -1) return null;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open, index + 1);
    }
  }
  return null;
}

export function verifiedGoodFailures(read) {
  const failures = [];

  const profile = read(PROFILE);
  const accept = functionBody(profile, "accept");
  const dismiss = functionBody(profile, "dismiss");

  if (!accept) {
    failures.push(
      `${PROFILE} must keep \`pub fn accept\`: accepting a changed value as the new baseline is the only road that moves good on a person's say-so.`,
    );
  }
  if (!dismiss) {
    failures.push(
      `${PROFILE} must keep \`pub fn dismiss\`: without it the only way to quiet a change would be to accept it, which is a different statement about the site.`,
    );
  }
  if (dismiss && /origin:\s*RecordOrigin::/.test(dismiss)) {
    failures.push(
      `${PROFILE} dismiss() must not write a good-value origin. Dismissing silences a change; it does not decide the change was correct, and a dismissal that rewrites good makes "stop telling me" mean "this is the new truth".`,
    );
  }
  if (dismiss && /good:\s*FieldRecord\s*\{/.test(dismiss)) {
    failures.push(
      `${PROFILE} dismiss() must not build a new good record. Good moves on exactly two roads: the value comes back, or a person accepts it.`,
    );
  }
  if (accept && !accept.includes("RecordOrigin::Accepted")) {
    failures.push(
      `${PROFILE} accept() must stamp RecordOrigin::Accepted. A baseline whose provenance does not say a person moved it cannot be audited later.`,
    );
  }
  for (const [name, body] of [
    ["accept", accept],
    ["dismiss", dismiss],
  ]) {
    if (body && !body.includes("self.guard(")) {
      failures.push(
        `${PROFILE} ${name}() must go through guard(): a decision is taken against a revision and a value digest, so a site that changed again mid-decision is refused instead of blessed unseen.`,
      );
    }
  }
  if (!/StaleRevision/.test(profile)) {
    failures.push(
      `${PROFILE} must keep the StaleRevision refusal; it is the closed-vocabulary code the connected acceptance endpoint answers with (409 stale_revision).`,
    );
  }

  const projection = read(PROJECTION);
  const fromHeaders = functionBody(projection, "from_headers");
  if (!fromHeaders) {
    failures.push(`${PROJECTION} must keep SecurityHeaderProfile::from_headers.`);
  } else {
    if (!fromHeaders.includes("SECURITY_HEADER_ALLOWLIST")) {
      failures.push(
        `${PROJECTION} from_headers must project through SECURITY_HEADER_ALLOWLIST. Iterating the response headers and filtering them out is a step someone can forget; walking the allowlist cannot include a header nobody listed.`,
      );
    }
    if (/headers\.iter\(\)|for\s*\(\s*name\s*,\s*value\s*\)\s*in\s*headers/.test(fromHeaders)) {
      failures.push(
        `${PROJECTION} from_headers must not walk the raw header map. Set-Cookie and every unlisted header must be structurally incapable of riding a stored baseline.`,
      );
    }
  }
  const allowlist = /SECURITY_HEADER_ALLOWLIST[^;]*;/s.exec(projection)?.[0] ?? "";
  for (const forbidden of ["set-cookie", "authorization", "cookie"]) {
    if (allowlist.includes(`"${forbidden}"`)) {
      failures.push(
        `${PROJECTION} must not allowlist "${forbidden}": credentials and session state have no place in a stored comparison value.`,
      );
    }
  }
  if (!projection.includes("TXT_POLICY_PREFIXES")) {
    failures.push(
      `${PROJECTION} must keep TXT_POLICY_PREFIXES. Arbitrary TXT records routinely hold third-party verification secrets, so only allowlisted policy strings may be stored.`,
    );
  }

  const siteFacts = read(SITE_FACTS);
  if (!siteFacts.includes("DNS_CHECK_PREFIX")) {
    failures.push(
      `${SITE_FACTS} must gate its DNS reads on the questions the scan already asked (DNS_CHECK_PREFIX). Recording a baseline is not a licence to open connections the person did not ask for.`,
    );
  }
  if (!/if\s+!results/.test(siteFacts)) {
    failures.push(
      `${SITE_FACTS} must check the scan's own results before asking a DNS question; the resolver cache makes a repeat question free, and a first question is new egress.`,
    );
  }

  const migration = read(MIGRATION);
  const originVariants = [...profile.matchAll(/Self::(\w+)\s*=>\s*"(\w+)"/g)].map(
    (match) => match[2],
  );
  for (const variant of originVariants) {
    // Only the origin vocabulary is CHECK-constrained; field keys are not.
    if (!["seeded", "promoted", "accepted", "reseeded"].includes(variant)) continue;
    if (!migration.includes(`'${variant}'`)) {
      failures.push(
        `${MIGRATION} CHECK constraint is missing '${variant}'. A record origin the schema rejects passes every test and fails on the user's disk.`,
      );
    }
  }
  if (!/drift_value_json/.test(migration) || !/good_value_json/.test(migration)) {
    failures.push(
      `${MIGRATION} must store the good value and the differing value in separate columns. One column would destroy the comparison the row exists to make.`,
    );
  }

  return failures;
}
