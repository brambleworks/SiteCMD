import { describe, expect, it } from "vitest";
import { parseAlertDetailRecord, parseDeployRegressionDetail } from "./alert-detail-model";

// Keep synchronized with DETAIL_FIXTURE in regression_blame_tests.rs.
const RUST_FIXTURE = `{"alert_type":"deploy_regression","scan_kind":"web","scan_id":42,"regression_id":7,"previous_score":92,"current_score":84,"score_drop":8,"new_issues":[{"check_id":"security.csp-header","title":"Missing Content-Security-Policy header"},{"check_id":"seo.meta-description","title":"Missing meta description"}],"fixed_count":1,"detector_changed_count":0,"engine_release":"1.5.4","commit_from":"aaa1111111","commit_to":"bbb2222222","commit_count":3,"commits":[{"hash":"bbb2222222","short_hash":"bbb2222","message":"Ship the redesign","author":"Kyle","date":"2026-06-09T12:00:00-05:00"}],"url":"https://example.com","destination":"issues"}`;

describe("parseDeployRegressionDetail", () => {
  it("parses the Rust writer fixture end to end", () => {
    const detail = parseDeployRegressionDetail(parseAlertDetailRecord(RUST_FIXTURE));
    if (!detail) throw new Error("expected the deploy_regression fixture to parse");

    expect(detail.scanKind).toBe("web");
    expect(detail.scoreDrop).toBe(8);
    expect(detail.newIssues).toEqual([
      { checkId: "security.csp-header", title: "Missing Content-Security-Policy header" },
      { checkId: "seo.meta-description", title: "Missing meta description" },
    ]);
    expect(detail.commitFrom).toBe("aaa1111111");
    expect(detail.commitCount).toBe(3);
    expect(detail.commits[0]?.shortHash).toBe("bbb2222");
    expect(detail.detectorChangedCount).toBe(0);
    expect(detail.engineRelease).toBe("1.5.4");
  });

  it("reads the findings the writer refused to attribute", () => {
    const withheld = RUST_FIXTURE.replace(
      '"detector_changed_count":0',
      '"detector_changed_count":2',
    );
    const detail = parseDeployRegressionDetail(parseAlertDetailRecord(withheld));
    if (!detail) throw new Error("expected the withheld-findings record to parse");

    expect(detail.detectorChangedCount).toBe(2);
  });

  it("defaults the attribution fields when an older record omits them", () => {
    const detail = parseDeployRegressionDetail({
      alert_type: "deploy_regression",
      regression_id: 7,
      commit_to: "bbb2222222",
    });
    if (!detail) throw new Error("expected the legacy record to parse");

    expect(detail.detectorChangedCount).toBe(0);
    expect(detail.engineRelease).toBe("");
  });

  it("passes a negative score_drop through unclamped", () => {
    // The Rust writer fires blame on a new critical/high finding even when
    // the score improved, so score_drop can be zero or negative.
    const improvedScoreFixture = RUST_FIXTURE.replace('"score_drop":8', '"score_drop":-3');
    const detail = parseDeployRegressionDetail(parseAlertDetailRecord(improvedScoreFixture));
    if (!detail) throw new Error("expected the improved-score blame record to parse");

    expect(detail.scoreDrop).toBe(-3);
  });

  it("returns null for other alert types", () => {
    expect(parseDeployRegressionDetail({ alert_type: "web_score_drop" })).toBeNull();
  });

  it("returns null for a corrupt record missing the writer invariants", () => {
    expect(parseDeployRegressionDetail({ alert_type: "deploy_regression" })).toBeNull();
    expect(parseDeployRegressionDetail(parseAlertDetailRecord(RUST_FIXTURE))).not.toBeNull();
  });

  it("tolerates malformed arrays without throwing", () => {
    const detail = parseDeployRegressionDetail({
      alert_type: "deploy_regression",
      regression_id: 7,
      new_issues: [null, { title: "no id" }],
      commits: "nope",
    });
    if (!detail) throw new Error("expected malformed detail to still parse");

    expect(detail.newIssues).toEqual([]);
    expect(detail.commits).toEqual([]);
  });
});
