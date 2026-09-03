//! Portable `seo.sitemap_freshness` verdict: grades direct `<lastmod>` usage
//! in an already-fetched sitemap document. The tag is optional per entry, so
//! coverage alone is never a defect; only malformed or repeated values warn.

use crate::checks::seo::sitemap::SitemapFetch;
use crate::checks::{CheckResult, CheckStatus, IssueConfidence, ScanCategory, Severity};

/// Grade the `seo.sitemap_freshness` outcome from the shared sitemap fetch.
/// An empty document yields no rows: `seo.sitemap` owns the empty-document
/// finding.
pub fn evaluate_sitemap_freshness(found: &SitemapFetch) -> Vec<CheckResult> {
    if found.entry_count == 0 {
        return vec![];
    }

    let lastmod = sitemap_lastmod_summary(&found.body);
    let invalid: Vec<String> = lastmod
        .values
        .iter()
        .filter(|value| !valid_sitemap_lastmod(value))
        .take(10)
        .map(|value| value.chars().take(100).collect())
        .collect();
    let invalid_count = lastmod
        .values
        .iter()
        .filter(|value| !valid_sitemap_lastmod(value))
        .count();
    let coverage =
        (lastmod.entries_with_lastmod as f64 / found.entry_count as f64 * 100.0).round() as u32;
    let invalid_values = invalid_count > 0;
    let repeated_values = lastmod.entries_with_multiple_lastmod > 0;
    let has_issue = invalid_values || repeated_values;

    vec![CheckResult {
        check_id: "seo.sitemap_freshness".into(),
        category: ScanCategory::Seo,
        title: if invalid_values && repeated_values {
            "Sitemap has invalid lastmod usage".into()
        } else if invalid_values {
            "Sitemap has invalid lastmod values".into()
        } else if repeated_values {
            "Sitemap entries repeat lastmod".into()
        } else {
            "Sitemap lastmod usage".into()
        },
        description: if has_issue {
            invalid_lastmod_description(
                invalid_count,
                lastmod.values.len(),
                lastmod.entries_with_multiple_lastmod,
            )
        } else if lastmod.values.is_empty() {
            format!(
                "None of the sitemap's {} entries includes the optional <lastmod> tag. That is valid: SiteCMD cannot infer trustworthy page-modification dates and does not recommend inventing them.",
                found.entry_count
            )
        } else {
            format!(
                "{} of {} sitemap entries include a syntactically valid <lastmod> value ({}%). The tag is optional per entry, and coverage alone is not a defect. This syntax check cannot confirm that the dates match significant page changes or that a search engine uses them.",
                lastmod.entries_with_lastmod,
                found.entry_count,
                coverage
            )
        },
        status: if has_issue {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        severity: Severity::Low,
        fix_prompt: None,
        manual_fix: if has_issue {
            Some("Emit at most one direct `<lastmod>` child per `<url>` or `<sitemap>` entry. Use a W3C date (`YYYY-MM-DD`) or date-time with a timezone, and derive it from that entry's last significant content change. Correct malformed or repeated values, or omit the optional tag when no trustworthy entry-specific date exists; do not stamp every URL with the build time.".into())
        } else {
            None
        },
        raw_data: Some(serde_json::json!({
            "entry_count": found.entry_count,
            "direct_lastmod_count": lastmod.values.len(),
            "entries_with_lastmod": lastmod.entries_with_lastmod,
            "entries_with_multiple_lastmod": lastmod.entries_with_multiple_lastmod,
            "coverage_pct": coverage,
            "invalid_lastmod_count": invalid_count,
            "invalid_lastmod_samples": invalid,
            "lastmod_optional": true,
            "semantic_accuracy_verified": false,
        })),
        confidence: IssueConfidence::High,
        confidence_reason: None,
        why_it_matters: if has_issue {
            Some("Consumers may ignore malformed or non-conforming freshness hints. Even valid dates are useful only when they consistently track meaningful changes to the linked content.".into())
        } else {
            None
        },
    }]
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SitemapLastmodSummary {
    values: Vec<String>,
    entries_with_lastmod: usize,
    entries_with_multiple_lastmod: usize,
}

fn invalid_lastmod_description(
    invalid_count: usize,
    direct_value_count: usize,
    repeated_entry_count: usize,
) -> String {
    let mut problems = Vec::new();
    if invalid_count > 0 {
        problems.push(format!(
            "{} of {} direct <lastmod> values do not use a W3C Datetime shape (YYYY, YYYY-MM, YYYY-MM-DD, or a date with a Thh:mm, Thh:mm:ss, or Thh:mm:ss.s time and a Z or +hh:mm zone designator)",
            invalid_count, direct_value_count
        ));
    }
    if repeated_entry_count > 0 {
        problems.push(format!(
            "{} sitemap {} more than one direct <lastmod> child",
            repeated_entry_count,
            if repeated_entry_count == 1 {
                "entry has"
            } else {
                "entries have"
            }
        ));
    }
    format!(
        "{}. The protocol permits at most one optional <lastmod> per <url> or <sitemap> entry. Any emitted value should be syntactically valid and reflect that entry's last significant modification.",
        problems.join("; ")
    )
}

fn sitemap_lastmod_summary(xml: &str) -> SitemapLastmodSummary {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    let mut summary = SitemapLastmodSummary::default();
    let mut depth = 0usize;
    let mut entry_depth = None::<usize>;
    let mut entry_lastmod_count = 0usize;
    let mut current = None::<(usize, String)>;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                depth += 1;
                let name = element.name();
                let local_name = name.local_name();
                let local_name = local_name.as_ref();
                if depth == 2 && matches!(local_name, b"url" | b"sitemap") {
                    entry_depth = Some(depth);
                    entry_lastmod_count = 0;
                } else if local_name == b"lastmod"
                    && entry_depth.is_some_and(|entry| depth == entry + 1)
                {
                    current = Some((depth, String::new()));
                }
            }
            Ok(Event::Empty(element)) => {
                let name = element.name();
                let local_name = name.local_name();
                if local_name.as_ref() == b"lastmod"
                    && entry_depth.is_some_and(|entry| depth + 1 == entry + 1)
                {
                    summary.values.push(String::new());
                    entry_lastmod_count += 1;
                }
            }
            Ok(Event::Text(text)) if current.is_some() => {
                current
                    .as_mut()
                    .expect("guarded above")
                    .1
                    .push_str(&String::from_utf8_lossy(text.as_ref()));
            }
            Ok(Event::CData(text)) if current.is_some() => {
                current
                    .as_mut()
                    .expect("guarded above")
                    .1
                    .push_str(&String::from_utf8_lossy(text.as_ref()));
            }
            Ok(Event::End(element)) => {
                let name = element.name();
                let local_name = name.local_name();
                let local_name = local_name.as_ref();
                if local_name == b"lastmod" && current.as_ref().is_some_and(|(d, _)| *d == depth) {
                    if let Some((_, value)) = current.take() {
                        summary.values.push(value.trim().to_string());
                        entry_lastmod_count += 1;
                    }
                }
                if entry_depth == Some(depth) && matches!(local_name, b"url" | b"sitemap") {
                    if entry_lastmod_count > 0 {
                        summary.entries_with_lastmod += 1;
                    }
                    if entry_lastmod_count > 1 {
                        summary.entries_with_multiple_lastmod += 1;
                    }
                    entry_depth = None;
                    entry_lastmod_count = 0;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    summary
}

/// The sitemap protocol references the W3C Datetime note, whose complete
/// profile is `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, and `YYYY-MM-DDThh:mm`,
/// `YYYY-MM-DDThh:mm:ss`, or `YYYY-MM-DDThh:mm:ss.s` followed by a zone
/// designator (`Z`, `+hh:mm`, or `-hh:mm`). A time without a zone is invalid.
fn valid_sitemap_lastmod(value: &str) -> bool {
    match value.split_once(['T', 't']) {
        None => valid_w3c_date(value, false),
        Some((date, time)) => valid_w3c_date(date, true) && valid_w3c_time_with_zone(time),
    }
}

fn valid_w3c_date(date: &str, require_day: bool) -> bool {
    if !date
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'-')
    {
        return false;
    }
    match date.len() {
        4 if !require_day => date.bytes().all(|byte| byte.is_ascii_digit()),
        7 if !require_day => {
            chrono::NaiveDate::parse_from_str(&format!("{date}-01"), "%Y-%m-%d").is_ok()
        }
        10 => chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(),
        _ => false,
    }
}

fn valid_w3c_time_with_zone(time: &str) -> bool {
    let local = match time.strip_suffix(['Z', 'z']) {
        Some(local) => local,
        None => match time.rfind(['+', '-']) {
            Some(index) if valid_w3c_zone_offset(&time[index + 1..]) => &time[..index],
            _ => return false,
        },
    };
    let (clock, fraction) = match local.split_once('.') {
        Some((clock, fraction)) => (clock, Some(fraction)),
        None => (local, None),
    };
    if fraction.is_some_and(|digits| {
        digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return false;
    }
    let format = match (clock.len(), fraction) {
        (5, None) => "%H:%M",
        (8, _) => "%H:%M:%S",
        _ => return false,
    };
    chrono::NaiveTime::parse_from_str(clock, format).is_ok()
}

/// `hh:mm` after the sign: hours 00-23, minutes 00-59.
fn valid_w3c_zone_offset(offset: &str) -> bool {
    let Some((hours, minutes)) = offset.split_once(':') else {
        return false;
    };
    let two_digits = |part: &str| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit());
    two_digits(hours)
        && two_digits(minutes)
        && hours.parse::<u8>().is_ok_and(|hours| hours <= 23)
        && minutes.parse::<u8>().is_ok_and(|minutes| minutes <= 59)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::seo::sitemap::{parse_sitemap_document, SitemapParse};

    fn fetch(xml: &str) -> SitemapFetch {
        let SitemapParse::WellFormed(document) = parse_sitemap_document(xml) else {
            panic!("test fixture must be a valid sitemap document");
        };
        SitemapFetch::new("https://example.com/sitemap.xml", xml, &document)
    }

    fn urlset(url_count: usize, lastmod_count: usize) -> String {
        let mut xml = String::from("<urlset>");
        for i in 0..url_count {
            xml.push_str("<url><loc>https://example.com/p</loc>");
            if i < lastmod_count {
                xml.push_str("<lastmod>2026-07-01</lastmod>");
            }
            xml.push_str("</url>");
        }
        xml.push_str("</urlset>");
        xml
    }

    #[test]
    fn sitemap_without_any_lastmod_passes_because_the_tag_is_optional() {
        let results = evaluate_sitemap_freshness(&fetch(&urlset(3, 0)));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].description.contains("optional"));
        assert!(results[0]
            .description
            .contains("does not recommend inventing"));
    }

    #[test]
    fn partial_lastmod_coverage_passes_with_contextual_math() {
        let results = evaluate_sitemap_freshness(&fetch(&urlset(4, 1)));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].description.contains("25%"));
        assert!(results[0]
            .description
            .contains("coverage alone is not a defect"));
    }

    #[test]
    fn full_lastmod_coverage_passes() {
        let results = evaluate_sitemap_freshness(&fetch(&urlset(2, 2)));
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert!(results[0].description.contains("100%"));
        assert!(results[0]
            .description
            .contains("cannot confirm that the dates match"));
    }

    #[test]
    fn malformed_lastmod_values_warn_with_direct_evidence() {
        let xml =
            "<urlset><url><loc>https://example.com/</loc><lastmod>yesterday</lastmod></url></urlset>";
        let results = evaluate_sitemap_freshness(&fetch(xml));
        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(results[0].severity, Severity::Low);
        assert!(results[0].title.contains("invalid lastmod"));
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["invalid_lastmod_count"],
            1
        );
        assert!(results[0]
            .manual_fix
            .as_deref()
            .unwrap_or_default()
            .contains("omit the optional tag"));
    }

    #[test]
    fn nested_extension_lastmod_does_not_count_as_entry_freshness() {
        let xml = r#"<urlset xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
            <url>
                <loc>https://example.com/</loc>
                <image:image>
                    <image:loc>https://example.com/hero.jpg</image:loc>
                    <image:lastmod>2026-07-01</image:lastmod>
                </image:image>
            </url>
        </urlset>"#;
        let results = evaluate_sitemap_freshness(&fetch(xml));
        let raw = results[0].raw_data.as_ref().unwrap();

        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(raw["direct_lastmod_count"], 0);
        assert_eq!(raw["entries_with_lastmod"], 0);
        assert!(results[0]
            .description
            .contains("None of the sitemap's 1 entries"));
    }

    #[test]
    fn duplicate_direct_lastmod_tags_never_push_coverage_above_one_hundred() {
        let xml = r#"<urlset><url>
            <loc>https://example.com/</loc>
            <lastmod>2026-07-01</lastmod>
            <lastmod>2026-07-02T12:00:00Z</lastmod>
        </url></urlset>"#;
        let results = evaluate_sitemap_freshness(&fetch(xml));
        let raw = results[0].raw_data.as_ref().unwrap();

        assert_eq!(results[0].status, CheckStatus::Warn);
        assert_eq!(raw["direct_lastmod_count"], 2);
        assert_eq!(raw["entries_with_lastmod"], 1);
        assert_eq!(raw["entries_with_multiple_lastmod"], 1);
        assert_eq!(raw["coverage_pct"], 100);
        assert!(results[0]
            .description
            .contains("1 sitemap entry has more than one direct <lastmod> child"));
    }

    #[test]
    fn empty_sitemap_document_yields_no_freshness_result() {
        assert!(evaluate_sitemap_freshness(&fetch("<urlset></urlset>")).is_empty());
    }

    #[test]
    fn every_w3c_datetime_profile_shape_is_a_valid_lastmod() {
        for value in [
            "2026",
            "2026-09",
            "2026-09-02",
            "2026-09-02T03:00Z",
            "2026-09-02T03:00+02:00",
            "2026-09-02T03:00:00Z",
            "2026-09-02T03:00:00-05:00",
            "2026-09-02T03:00:00.5Z",
            "2026-09-02T03:00:00.123456+00:00",
            "2026-09-02t03:00:00z",
        ] {
            assert!(
                valid_sitemap_lastmod(value),
                "{value} is in the W3C profile"
            );
        }
    }

    #[test]
    fn shapes_outside_the_w3c_profile_are_invalid_lastmod_values() {
        for value in [
            "2026-09-02 03:00",
            "2026-13-01",
            "2026-09-02T03:00",
            "2026-09-02T03:00:00",
            "2026-09-02T25:00Z",
            "2026-09-02T03:00:00+24:00",
            "2026-09-02T03:00:00.Z",
            "2026-9-2",
            "20260902",
            "2026-02-30",
            "yesterday",
            "",
        ] {
            assert!(
                !valid_sitemap_lastmod(value),
                "{value} is outside the profile"
            );
        }
    }

    #[test]
    fn minute_precision_zoned_lastmod_values_pass_freshness() {
        let xml = r#"<urlset>
            <url><loc>https://example.com/</loc><lastmod>2026-09-02T03:00Z</lastmod></url>
            <url><loc>https://example.com/blog</loc><lastmod>2026-08-20T21:25Z</lastmod></url>
        </urlset>"#;
        let results = evaluate_sitemap_freshness(&fetch(xml));
        assert_eq!(
            results[0].status,
            CheckStatus::Pass,
            "{}",
            results[0].description
        );
        assert_eq!(
            results[0].raw_data.as_ref().unwrap()["invalid_lastmod_count"],
            0
        );
    }
}
