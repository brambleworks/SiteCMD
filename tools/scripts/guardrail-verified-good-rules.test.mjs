import { describe, expect, it } from "vitest";
import { verifiedGoodFailures } from "./lib/guardrail-verified-good-rules.mjs";

const PROFILE = "apps/desktop/src-tauri/crates/engine/src/profile/mod.rs";
const PROJECTION = "apps/desktop/src-tauri/crates/engine/src/profile/projection.rs";
const SITE_FACTS = "apps/desktop/src-tauri/src/core/scanner/site_facts.rs";
const MIGRATION = "apps/desktop/src-tauri/src/db/migrations/018_verified_good_profile.sql";
const CARD = "apps/desktop/src/components/dashboard/zones/SiteBaselineCard.tsx";

const HEALTHY = {
  [PROFILE]: `
pub enum DecisionError { NoDrift, StaleRevision { current_revision: u64 } }
impl VerifiedGoodProfile {
    pub fn accept(&self, field: ProfileField) -> Result<ProfileUpdate, DecisionError> {
        let state = self.guard(field, based_on_revision, expected_digest)?;
        profile.fields.insert(field, FieldState { good: FieldRecord { origin: RecordOrigin::Accepted } });
        Ok(update)
    }
    pub fn dismiss(&self, field: ProfileField) -> Result<ProfileUpdate, DecisionError> {
        let state = self.guard(field, based_on_revision, expected_digest)?;
        drift.dismissed = true;
        Ok(update)
    }
}
impl RecordOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seeded => "seeded",
            Self::Promoted => "promoted",
            Self::Accepted => "accepted",
            Self::Reseeded => "reseeded",
        }
    }
}
`,
  [PROJECTION]: `
pub const SECURITY_HEADER_ALLOWLIST: &[&str] = &["cache-control", "x-frame-options"];
const TXT_POLICY_PREFIXES: &[&str] = &["v=spf1", "v=DMARC1"];
impl SecurityHeaderProfile {
    pub fn from_headers(headers: &http::HeaderMap) -> Self {
        for name in SECURITY_HEADER_ALLOWLIST {
            let lines = headers.get_all(*name);
        }
        Self { headers: projected }
    }
}
`,
  [SITE_FACTS]: `
const DNS_CHECK_PREFIX: &str = "security.dns.";
async fn dns_posture(ctx: &CheckContext, results: &[CheckResult]) -> Option<DnsPosture> {
    if !results.iter().any(|r| r.check_id.starts_with(DNS_CHECK_PREFIX)) {
        return None;
    }
    None
}
`,
  [MIGRATION]: `
CREATE TABLE site_verified_good (
    good_value_json TEXT NOT NULL,
    good_origin TEXT NOT NULL
        CHECK(good_origin IN ('seeded', 'promoted', 'accepted', 'reseeded')),
    drift_value_json TEXT
);
`,
  [CARD]: `
const [confirming, setConfirming] = useState(null);
{confirming ? <Button onClick={() => onDecide(true)}>Accept as baseline</Button> : null}
`,
};

function failuresWith(overrides = {}) {
  const files = { ...HEALTHY, ...overrides };
  return verifiedGoodFailures((file) => {
    if (!(file in files)) throw new Error(`no fixture for ${file}`);
    return files[file];
  });
}

describe("dismissing a change is not accepting it", () => {
  it("passes when every rule holds", () => {
    expect(failuresWith()).toEqual([]);
  });

  it("fails when dismissing writes a new good value", () => {
    const dismissed = HEALTHY[PROFILE].replace(
      "        drift.dismissed = true;",
      "        good: FieldRecord { value: drift.value },",
    );
    expect(failuresWith({ [PROFILE]: dismissed }).join(" ")).toContain(
      "must not build a new good record",
    );
  });

  it("fails when dismissing stamps a good-value origin", () => {
    const dismissed = HEALTHY[PROFILE].replace(
      "        drift.dismissed = true;",
      "        origin: RecordOrigin::Accepted,",
    );
    expect(failuresWith({ [PROFILE]: dismissed }).join(" ")).toContain(
      "must not write a good-value origin",
    );
  });

  it("fails when accepting stops recording that a person moved the baseline", () => {
    const unstamped = HEALTHY[PROFILE].replace("RecordOrigin::Accepted", "RecordOrigin::Promoted");
    expect(failuresWith({ [PROFILE]: unstamped }).join(" ")).toContain(
      "must stamp RecordOrigin::Accepted",
    );
  });

  it("fails when a decision skips the revision and digest guard", () => {
    const ungated = HEALTHY[PROFILE].replace(
      "        let state = self.guard(field, based_on_revision, expected_digest)?;\n        drift.dismissed = true;",
      "        drift.dismissed = true;",
    );
    expect(failuresWith({ [PROFILE]: ungated }).join(" ")).toContain("must go through guard()");
  });
});

describe("a baseline holds only what it is allowed to hold", () => {
  it("fails when the header projection walks the raw header map", () => {
    const filtered = HEALTHY[PROJECTION].replace(
      "for name in SECURITY_HEADER_ALLOWLIST {\n            let lines = headers.get_all(*name);",
      "for (name, value) in headers.iter() {",
    );
    const messages = failuresWith({ [PROJECTION]: filtered }).join(" ");
    expect(messages).toContain("must project through SECURITY_HEADER_ALLOWLIST");
    expect(messages).toContain("must not walk the raw header map");
  });

  it("fails when a credential header joins the allowlist", () => {
    const leaky = HEALTHY[PROJECTION].replace('"cache-control"', '"set-cookie"');
    expect(failuresWith({ [PROJECTION]: leaky }).join(" ")).toContain(
      'must not allowlist "set-cookie"',
    );
  });

  it("fails when arbitrary TXT records can ride", () => {
    const leaky = HEALTHY[PROJECTION].replace("TXT_POLICY_PREFIXES", "TXT_EVERYTHING");
    expect(failuresWith({ [PROJECTION]: leaky }).join(" ")).toContain("TXT_POLICY_PREFIXES");
  });
});

describe("recording a baseline never widens a scan's egress", () => {
  it("fails when the DNS read stops checking what the scan already asked", () => {
    const ungated = HEALTHY[SITE_FACTS].replace(
      "    if !results.iter().any(|r| r.check_id.starts_with(DNS_CHECK_PREFIX)) {\n        return None;\n    }\n",
      "",
    );
    expect(failuresWith({ [SITE_FACTS]: ungated }).join(" ")).toContain(
      "must check the scan's own results",
    );
  });
});

describe("the schema and the code agree", () => {
  it("fails when a record origin the schema rejects is added", () => {
    const missing = HEALTHY[MIGRATION].replace(", 'reseeded'", "");
    expect(failuresWith({ [MIGRATION]: missing }).join(" ")).toContain("is missing 'reseeded'");
  });

  it("fails when good and the differing value share a column", () => {
    const collapsed = HEALTHY[MIGRATION].replace("    drift_value_json TEXT\n", "");
    expect(failuresWith({ [MIGRATION]: collapsed }).join(" ")).toContain("separate columns");
  });
});

describe("accepting asks first", () => {
  it("fails when the card drops its confirmation step", () => {
    const unconfirmed = `<Button onClick={() => onDecide(true)}>Accept as baseline</Button>`;
    const messages = failuresWith({ [CARD]: unconfirmed }).join(" ");
    expect(messages).toContain("must confirm before accepting");
    expect(messages).toContain("outside the confirmation branch");
  });
});
