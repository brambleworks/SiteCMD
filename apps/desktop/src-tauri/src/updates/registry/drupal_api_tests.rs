//! Tests for the Drupal.org registry client.

use super::*;
use crate::updates::types::UpdateType;

#[test]
fn module_name_strips_drupal_prefix() {
    assert_eq!(module_name_from_package("drupal/views"), "views");
    assert_eq!(module_name_from_package("drupal/webform"), "webform");
}

#[test]
fn module_name_passes_through_unprefixed() {
    // Some lockfile shapes already store the bare name.
    assert_eq!(module_name_from_package("views"), "views");
    assert_eq!(module_name_from_package(""), "");
}

#[test]
fn is_security_release_detects_term_marker() {
    assert!(is_security_release("<term>Security update</term>"));
}

#[test]
fn is_security_release_detects_release_type_value_marker() {
    assert!(is_security_release(
        "<terms><term><name>Release type</name><value>Security update</value></term></terms>"
    ));
}

#[test]
fn is_security_release_ignores_coverage_boilerplate() {
    assert!(!is_security_release(
        "<security covered=\"1\">Covered by Drupal's security advisory policy</security>"
    ));
}

#[test]
fn is_security_release_ignores_bare_sa_id_in_prose() {
    // A past advisory id in release notes is not itself a security release;
    // the authoritative signal is the "Security update" release-type term.
    assert!(!is_security_release(
        "Bug fixes; see SA-CONTRIB-2024-001 for the earlier issue."
    ));
}

#[test]
fn is_security_release_returns_false_for_normal_release() {
    assert!(!is_security_release("<term>Bug fixes</term>"));
    assert!(!is_security_release(""));
}

#[test]
fn extract_latest_version_picks_first_stable_in_release_block() {
    let xml = r#"<project>
<releases>
<release>
<version>2.5.0</version>
</release>
<release>
<version>2.4.0</version>
</release>
</releases>
</project>"#;
    assert_eq!(extract_latest_version(xml).as_deref(), Some("2.5.0"));
}

#[test]
fn extract_latest_version_skips_pre_release_tags() {
    // SECURITY/UX: dev/alpha/beta/rc must be skipped so production
    // Drupal sites don't get auto-updated to a pre-release.
    let xml = r#"<release>
<version>3.0.0-dev</version>
</release>
<release>
<version>3.0.0-alpha1</version>
</release>
<release>
<version>3.0.0-beta2</version>
</release>
<release>
<version>3.0.0-rc1</version>
</release>
<release>
<version>2.5.0</version>
</release>"#;
    assert_eq!(extract_latest_version(xml).as_deref(), Some("2.5.0"));
}

#[test]
fn extract_latest_version_returns_none_when_only_pre_releases() {
    let xml = r#"<release><version>1.0.0-dev</version></release>
<release><version>1.0.0-alpha</version></release>"#;
    assert!(extract_latest_version(xml).is_none());
}

#[test]
fn extract_latest_version_returns_none_for_empty_xml() {
    assert!(extract_latest_version("").is_none());
    assert!(extract_latest_version("<project></project>").is_none());
}

#[test]
fn extract_latest_version_handles_indented_xml() {
    // Real Drupal feed responses are pretty-printed; the parser must
    // tolerate leading whitespace on every line.
    let xml = r#"  <release>
    <version>9.5.10</version>
  </release>"#;
    assert_eq!(extract_latest_version(xml).as_deref(), Some("9.5.10"));
}

#[test]
fn recommended_release_skips_a_release_flagged_insecure() {
    let xml = r#"<project><releases>
<release>
<version>2.6.0</version>
<terms><term><name>Release type</name><value>Insecure</value></term></terms>
</release>
<release>
<version>2.5.0</version>
<terms><term><name>Release type</name><value>Bug fixes</value></term></terms>
</release>
</releases></project>"#;
    assert_eq!(extract_latest_version(xml).as_deref(), Some("2.5.0"));
}

#[test]
fn extract_latest_version_handles_single_line_feed() {
    let xml = "<project><releases><release><version>2.5.0</version></release><release><version>2.4.0</version></release></releases></project>";
    assert_eq!(extract_latest_version(xml).as_deref(), Some("2.5.0"));
}

// The parser scans by `<release>` boundaries and is whitespace-agnostic,
// so fixtures may be pretty-printed or compact; these use newlines for
// readability.
fn release_xml(version: &str) -> String {
    format!("<release>\n<version>{}</version>\n</release>", version)
}

#[test]
fn build_update_returns_some_for_upgrade() {
    let body = release_xml("2.5.0");
    let update = build_update_from_response("drupal/views", "2.4.0", "composer.json", &body)
        .expect("update");
    assert_eq!(update.name, "drupal/views");
    assert_eq!(update.latest_version, "2.5.0");
    assert_eq!(update.ecosystem, Ecosystem::Drupal);
    assert_eq!(update.update_type, UpdateType::Minor);
    assert!(!update.is_security);
    assert!(update.advisory_url.is_none());
    assert!(update.advisory_severity.is_none());
    assert!(
        !update.is_dev,
        "Drupal updates are always reported as non-dev"
    );
}

#[test]
fn build_update_marks_security_release() {
    // SECURITY: a security release MUST set is_security=true and
    // populate advisory_url so the user sees the high-priority badge.
    let body = "<release>\n<version>2.5.1</version>\n<term>Security update</term>\n</release>";
    let update =
        build_update_from_response("drupal/views", "2.5.0", "composer.json", body).expect("update");
    assert!(update.is_security);
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
    assert_eq!(
        update.advisory_url.as_deref(),
        Some("https://www.drupal.org/project/views/releases"),
        "advisory URL must use the bare module name (no drupal/ prefix)",
    );
}

#[test]
fn unorderable_installed_version_keeps_the_security_flag() {
    let body = "<release>\n<version>2.5.1</version>\n<term>Security update</term>\n</release>";
    let update = build_update_from_response("drupal/views", "8.x-1.x-dev", "composer.json", body)
        .expect("update");
    assert!(update.is_security);
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
}

#[test]
fn routine_update_is_not_security_even_when_history_has_a_past_sa() {
    let body = r#"<project>
<releases>
<release>
<version>2.6.0</version>
<terms><term><name>Release type</name><value>Bug fixes</value></term></terms>
<security covered="1">Covered by Drupal's security advisory policy</security>
</release>
<release>
<version>2.5.1</version>
<terms><term><name>Release type</name><value>Security update</value></term></terms>
</release>
</releases>
</project>"#;
    let update =
        build_update_from_response("drupal/token", "2.5.1", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "2.6.0");
    assert!(
        !update.is_security,
        "routine 2.6.0 must not inherit the older 2.5.1 security release"
    );
    assert!(update.advisory_url.is_none());
    assert!(update.advisory_severity.is_none());
}

// Real drupal.org feeds serialize each <release> on one line; these
// fixtures match that compact shape.

#[test]
fn drupal_version_orders_within_and_across_schemes() {
    let v = |s: &str| parse_drupal_version(s).expect(s);
    assert!(
        v("8.x-1.10") > v("8.x-1.9"),
        "legacy suffixes compare numerically, not as strings"
    );
    assert!(v("8.x-1.5") < v("8.x-2.0"));
    assert!(v("2.5.1") > v("2.5.0"));
    assert!(
        v("2.0.0") > v("8.x-1.5"),
        "semver postdates the legacy scheme"
    );
    assert_eq!(v("2.5"), v("2.5.0"));
}

#[test]
fn same_branch_requires_same_scheme_and_same_first_part() {
    let v = |s: &str| parse_drupal_version(s).expect(s);
    assert!(v("2.5.1").same_branch(&v("2.0")));
    assert!(!v("2.5.1").same_branch(&v("3.0.2")));
    assert!(v("8.x-2.5").same_branch(&v("8.x-2.9")));
    assert!(!v("8.x-1.5").same_branch(&v("8.x-2.5")));
    assert!(
        !v("2.5.0").same_branch(&v("8.x-2.5")),
        "matching branch numbers across schemes are still different lines"
    );
}

#[test]
fn unorderable_versions_parse_to_none() {
    assert!(parse_drupal_version("8.x-2.x-dev").is_none());
    assert!(parse_drupal_version("2.0.0-beta1").is_none());
    assert!(parse_drupal_version("").is_none());
    assert!(parse_drupal_version(".x-1.0").is_none());
}

#[test]
fn upgrade_crossing_intermediate_security_release_is_flagged() {
    let body = "<project><releases><release><version>2.6.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms><security covered=\"1\">Covered by Drupal's security advisory policy</security></release><release><version>2.5.0</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>2.4.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.4.0", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "2.6.0");
    assert!(
        update.is_security,
        "upgrade crossing the intermediate 2.5.0 security release must be flagged"
    );
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
}

#[test]
fn security_release_older_than_installed_does_not_flag() {
    // 2.5.2 already includes the 2.5.1 fix; the routine 2.6.0 update
    // must not inherit the past advisory.
    let body = "<project><releases><release><version>2.6.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>2.5.1</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.5.2", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "2.6.0");
    assert!(!update.is_security);
    assert!(update.advisory_url.is_none());
}

#[test]
fn legacy_scheme_intermediate_security_release_is_flagged() {
    // Legacy ordering must be numeric: 8.x-1.5 < 8.x-1.9 < 8.x-1.10
    // (string comparison would put 8.x-1.10 before 8.x-1.9).
    let body = "<project><releases><release><version>8.x-1.10</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>8.x-1.9</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>8.x-1.5</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release></releases></project>";
    let update = build_update_from_response("drupal/views", "8.x-1.5", "composer.json", body)
        .expect("update");
    assert_eq!(update.latest_version, "8.x-1.10");
    assert!(
        update.is_security,
        "upgrade crossing the intermediate 8.x-1.9 security release must be flagged"
    );
}

#[test]
fn cross_scheme_security_release_is_not_counted() {
    let body = "<project><releases><release><version>2.5.1</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>2.0.0</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>8.x-1.5</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release></releases></project>";
    let update = build_update_from_response("drupal/views", "8.x-1.5", "composer.json", body)
        .expect("update");
    assert_eq!(update.latest_version, "2.5.1");
    assert!(
        !update.is_security,
        "the cross-scheme 2.0.0 fix must not flag the legacy install"
    );
    assert!(update.advisory_url.is_none());
}

#[test]
fn parallel_fix_on_another_scheme_does_not_flag_patched_install() {
    let body = "<project><releases><release><version>3.6.3</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>3.0.2</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>8.x-2.5</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/admin_toolbar", "8.x-2.5", "composer.json", body)
            .expect("update");
    assert_eq!(update.latest_version, "3.6.3");
    assert!(
        !update.is_security,
        "8.x-2.5 already carries the same-advisory fix"
    );
    assert!(update.advisory_url.is_none());
    assert!(update.advisory_severity.is_none());
}

#[test]
fn same_branch_security_release_flags_despite_other_branch_recommendation() {
    let body = "<project><releases><release><version>3.6.3</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>2.3.0</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.1.0", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "3.6.3");
    assert!(
        update.is_security,
        "the 2.x install is missing its own branch's 2.3.0 fix"
    );
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
}

#[test]
fn patched_install_is_not_flagged_when_recommended_is_another_branch() {
    let body = "<project><releases><release><version>3.6.3</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>2.5.2</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.5.3", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "3.6.3");
    assert!(!update.is_security);
    assert!(update.advisory_url.is_none());
}

#[test]
fn cross_branch_security_recommendation_does_not_flag_patched_install() {
    let body = "<project><releases><release><version>3.0.2</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>8.x-2.5</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/admin_toolbar", "8.x-2.5", "composer.json", body)
            .expect("update");
    assert_eq!(update.latest_version, "3.0.2");
    assert!(
        !update.is_security,
        "8.x-2.5 is the same-day parallel fix; the cross-scheme 3.0.2 recommendation must not flag it"
    );
    assert!(update.advisory_url.is_none());
    assert!(update.advisory_severity.is_none());
}

#[test]
fn same_branch_security_recommendation_still_flags_older_install() {
    // Polarity guard for the branch gate: a security release recommended on
    // the installed version's OWN branch must keep flagging an older install.
    let body = "<project><releases><release><version>2.5.0</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>2.4.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.4.0", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "2.5.0");
    assert!(
        update.is_security,
        "the same-branch 2.5.0 security recommendation must survive the branch gate"
    );
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
}

#[test]
fn installed_release_flagged_insecure_is_a_security_update() {
    let body = "<project><releases><release><version>3.6.3</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>8.x-1.5</version><terms><term><name>Release type</name><value>Insecure</value></term></terms></release></releases></project>";
    let update = build_update_from_response("drupal/views", "8.x-1.5", "composer.json", body)
        .expect("update");
    assert_eq!(update.latest_version, "3.6.3");
    assert!(
        update.is_security,
        "the Insecure stamp on the installed 8.x-1.5 release marks the install vulnerable"
    );
    assert_eq!(update.advisory_severity.as_deref(), Some("high"));
}

#[test]
fn unflagged_install_with_routine_recommendation_stays_routine() {
    let body = "<project><releases><release><version>3.6.3</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>8.x-1.5</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>8.x-1.4</version><terms><term><name>Release type</name><value>Insecure</value></term></terms></release></releases></project>";
    let update = build_update_from_response("drupal/views", "8.x-1.5", "composer.json", body)
        .expect("update");
    assert_eq!(update.latest_version, "3.6.3");
    assert!(!update.is_security);
    assert!(update.advisory_url.is_none());
}

#[test]
fn unparseable_history_version_is_excluded_without_flagging() {
    // A dev snapshot in the history is unorderable; it must be skipped
    // conservatively (no panic, no security flag), not guessed at.
    let body = "<project><releases><release><version>2.6.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release><release><version>8.x-2.x-dev</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release><release><version>2.4.0</version><terms><term><name>Release type</name><value>Bug fixes</value></term></terms></release></releases></project>";
    let update =
        build_update_from_response("drupal/token", "2.4.0", "composer.json", body).expect("update");
    assert_eq!(update.latest_version, "2.6.0");
    assert!(!update.is_security);
}

#[test]
fn unorderable_installed_version_disables_the_range_scan() {
    let body = "<project><releases><release><version>2.6.0</version><terms><term><name>Release type</name><value>Security update</value></term></terms></release></releases></project>";
    assert!(!history_has_security_release_in_range(
        body,
        "8.x-1.x-dev",
        "2.6.0"
    ));
    assert!(history_has_security_release_in_range(
        body, "2.4.0", "2.6.0"
    ));
}

#[test]
fn build_update_returns_none_when_versions_match() {
    let body = release_xml("2.5.0");
    let result = build_update_from_response("drupal/views", "2.5.0", "composer.json", &body);
    assert!(result.is_none());
}

#[test]
fn build_update_returns_none_when_no_stable_release() {
    let body = "<release>\n<version>3.0.0-dev</version>\n</release>";
    let result = build_update_from_response("drupal/views", "2.4.0", "composer.json", body);
    assert!(result.is_none());
}

#[test]
fn build_update_returns_none_for_empty_body() {
    let result = build_update_from_response("drupal/views", "2.4.0", "composer.json", "");
    assert!(result.is_none());
}

#[test]
fn build_update_handles_unprefixed_module_name() {
    let body = release_xml("2.5.0");
    let update =
        build_update_from_response("views", "2.4.0", "composer.json", &body).expect("update");
    assert_eq!(update.name, "views");
    assert_eq!(update.latest_version, "2.5.0");
}

#[test]
fn unsupported_project_is_flagged_deprecated_with_message() {
    let body = format!(
        "<project_status>unsupported</project_status>\n{}",
        release_xml("2.5.0")
    );
    let update = build_update_from_response("drupal/views", "2.4.0", "composer.json", &body)
        .expect("update");
    assert!(update.is_deprecated);
    assert_eq!(
        update.deprecation_message.as_deref(),
        Some("The Drupal project is marked unsupported on drupal.org.")
    );
    // The version upgrade info is still intact alongside the flag.
    assert_eq!(update.latest_version, "2.5.0");
    assert_eq!(update.update_type, UpdateType::Minor);
}

#[test]
fn revoked_project_is_flagged_deprecated_with_message() {
    let body = format!(
        "<project_status>revoked</project_status>\n{}",
        release_xml("2.5.0")
    );
    let update = build_update_from_response("drupal/views", "2.4.0", "composer.json", &body)
        .expect("update");
    assert!(update.is_deprecated);
    assert_eq!(
        update.deprecation_message.as_deref(),
        Some("The Drupal project is marked revoked on drupal.org.")
    );
}

#[test]
fn unsupported_project_surfaces_even_when_no_newer_version() {
    // Mirrors the npm rule: an entry must exist so the UI can show the
    // deprecation even though there is nothing to update to.
    let body = format!(
        "<project_status>unsupported</project_status>\n{}",
        release_xml("2.5.0")
    );
    let update = build_update_from_response("drupal/views", "2.5.0", "composer.json", &body)
        .expect("standalone deprecated entry");
    assert!(update.is_deprecated);
    assert_eq!(update.current_version, "2.5.0");
    assert_eq!(update.latest_version, "2.5.0");
    assert_eq!(update.update_type, UpdateType::Unknown);
}

#[test]
fn unsupported_project_without_stable_release_anchors_to_current() {
    let body = "<project_status>unsupported</project_status>";
    let update = build_update_from_response("drupal/views", "2.4.0", "composer.json", body)
        .expect("standalone deprecated entry");
    assert_eq!(update.latest_version, "2.4.0");
    assert_eq!(update.update_type, UpdateType::Unknown);
}

#[test]
fn published_project_status_is_not_deprecated() {
    let body = format!(
        "<project_status>published</project_status>\n{}",
        release_xml("2.5.0")
    );
    let update = build_update_from_response("drupal/views", "2.4.0", "composer.json", &body)
        .expect("update");
    assert!(!update.is_deprecated);
    assert!(update.deprecation_message.is_none());
}

#[test]
fn deprecation_only_entry_does_not_claim_security() {
    let body = format!(
        "<project_status>revoked</project_status>\nSA-CONTRIB-2024-001\n{}",
        release_xml("2.5.0")
    );
    let update = build_update_from_response("drupal/views", "2.5.0", "composer.json", &body)
        .expect("standalone deprecated entry");
    assert!(!update.is_security);
    assert!(update.advisory_severity.is_none());
    assert!(update.advisory_url.is_none());
}
