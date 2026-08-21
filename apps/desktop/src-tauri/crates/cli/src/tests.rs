use super::*;

#[test]
fn gate_threshold_consumes_exactly_one_value() {
    let args = parse_gate_args(
        [
            "--connection-export",
            "connection.sitecmd",
            "--threshold",
            "medium",
            "--strict",
            "--path",
            ".",
        ]
        .into_iter()
        .map(str::to_string),
    )
    .expect("valid gate arguments");

    assert_eq!(args.threshold, "medium");
    assert!(args.strict);
    assert_eq!(args.project_path, Some(".".into()));
}

#[test]
fn gate_threshold_is_valid_as_the_last_option() {
    let args = parse_gate_args(
        [
            "--connection-export",
            "connection.sitecmd",
            "--threshold",
            "high",
        ]
        .into_iter()
        .map(str::to_string),
    )
    .expect("valid gate arguments");

    assert_eq!(args.threshold, "high");
}

#[test]
fn web_scan_rejects_unknown_or_ignored_category_filters() {
    let unknown = parse_scan_args(
        ["--categories", "security,typo"]
            .into_iter()
            .map(str::to_string),
    )
    .err()
    .expect("unknown category must not produce a partial quality gate");
    assert!(unknown.contains("Unknown Web Scan category"));

    let ignored = parse_scan_args(
        ["--type", "security", "--categories", "security"]
            .into_iter()
            .map(str::to_string),
    )
    .err()
    .expect("focused scans must not silently ignore category filters");
    assert!(ignored.contains("only be used with --type health"));
}

#[cfg(not(feature = "browser"))]
#[test]
fn headless_release_rejects_browser_only_flags() {
    let error = parse_scan_args(["--cwv"].into_iter().map(str::to_string))
        .err()
        .expect("headless release cannot promise a browser measurement");
    assert!(error.contains("browser-enabled source build"));
}
