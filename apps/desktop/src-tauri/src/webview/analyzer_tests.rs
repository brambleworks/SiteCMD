use super::{browser_build_from_user_agent, NavigationGate, READY_PROBE_SCRIPT};
use sitecmd_engine::browser::DocumentMismatch;
use url::Url;

fn parse(url: &str) -> Url {
    Url::parse(url).expect("test url")
}

#[test]
fn gate_refuses_private_literals_inline_without_deferring() {
    let (gate, mut deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    for url in [
        "http://169.254.169.254/latest/meta-data/",
        "http://192.168.1.1/",
        "http://10.0.0.5/",
        "http://172.16.0.1/",
        "http://127.0.0.1:3000/",
        "http://[::1]:3000/",
        "http://localhost:3000/",
        "http://metadata.google.internal/",
    ] {
        assert!(!gate.decide(&parse(url)), "{url}");
    }
    assert!(
        deferred.try_recv().is_err(),
        "literals and local names are decided inline, never deferred to DNS"
    );
}

#[test]
fn gate_allows_the_origin_and_defers_unknown_hosts_until_dns_admits_them() {
    let (gate, mut deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    assert!(gate.decide(&parse("https://example.com/page")));
    assert!(gate.decide(&parse("https://EXAMPLE.com./other")));
    assert!(gate.decide(&parse("about:blank")));

    assert!(!gate.decide(&parse("https://cdn.example.net/")));
    assert_eq!(
        deferred.try_recv().expect("deferred hop").as_str(),
        "https://cdn.example.net/"
    );
    gate.allow_host("cdn.example.net");
    assert!(gate.decide(&parse("https://cdn.example.net/")));
}

#[tokio::test]
async fn dns_admission_validates_before_allowing() {
    let (gate, _deferred) = NavigationGate::new(&parse("https://example.com/"), false);
    assert!(gate
        .admit_after_dns(&parse("http://10.0.0.5/"))
        .await
        .is_err());
    assert!(gate
        .admit_after_dns(&parse("http://localhost/"))
        .await
        .is_err());
    assert!(!gate.decide(&parse("http://localhost/")));
}

#[test]
fn readiness_probe_never_fires_on_the_blank_start_page() {
    assert!(READY_PROBE_SCRIPT.contains("location.href !== 'about:blank'"));
    assert!(READY_PROBE_SCRIPT.contains("document.readyState === 'complete'"));
}

#[test]
fn explicit_local_scan_keeps_loopback_navigation() {
    let (gate, _deferred) = NavigationGate::new(&parse("http://localhost:3000/"), true);
    assert!(gate.decide(&parse("http://127.0.0.1:3000/")));
    assert!(gate.decide(&parse("http://localhost:3000/")));
    assert!(!gate.decide(&parse("http://192.168.1.1/")));
}

#[test]
fn browser_build_is_derived_from_the_runtime_user_agent() {
    assert_eq!(
        browser_build_from_user_agent(
            "webkit",
            "Mozilla/5.0 AppleWebKit/621.1.15 (KHTML, like Gecko) Version/18.5 Safari/621.1.15",
        )
        .as_deref(),
        Some("621.1.15")
    );
    assert_eq!(
        browser_build_from_user_agent(
            "webview2",
            "Mozilla/5.0 Chrome/136.0.7103.49 Safari/537.36 Edg/136.0.3240.50",
        )
        .as_deref(),
        Some("136.0.3240.50")
    );
}

/// Tauri mirrors `document.title` into the window only through
/// `on_document_title_changed`; without it `WebviewWindow::title()` holds the
/// static window title forever and every read-back silently times out, which
/// is how Core Web Vitals, the browser build, and axe went missing from every
/// scan before the bridge existed. The JSON payloads then have to cross in
/// chunks: the platform truncates a title to 1000 characters, so writing a
/// whole axe report into one title is how axe stayed missing afterwards.
#[test]
fn analyzer_reads_document_titles_through_the_bridge() {
    let source = include_str!("analyzer.rs");
    assert!(
        source.contains(".on_document_title_changed("),
        "analyzer must register the document-title handler"
    );
    assert!(
        !source.contains(".title()"),
        "analyzer must not poll WebviewWindow::title(); read TitleBridge instead"
    );
    for marker in ["READY_TITLE_PREFIX", "BROWSER_UA_TITLE_PREFIX"] {
        assert!(
            source.contains(&format!("read_prefixed({marker})")),
            "{marker} must be read through the bridge"
        );
    }
    for (global, marker) in [
        ("CWV_RESULT_GLOBAL", "CWV_TITLE_MARKER"),
        ("AXE_RESULT_GLOBAL", "AXE_TITLE_MARKER"),
    ] {
        assert!(
            source.contains(&format!("{global},\n        {marker},")),
            "{global} must be read through read_bridged_json under {marker}"
        );
    }
    assert!(
        !source.contains("JSON.stringify("),
        "a JSON payload crosses the bridge in chunks, never as one title"
    );
}

#[tokio::test]
async fn until_cancelled_returns_the_work_value_while_the_scan_runs() {
    let cancel = || false;
    let value = super::until_cancelled(&cancel, async { 7u32 })
        .await
        .expect("an uncancelled wait must deliver its value");
    assert_eq!(value, 7);
}

#[tokio::test]
async fn until_cancelled_refuses_before_polling_an_already_cancelled_wait() {
    let cancel = || true;
    let outcome = super::until_cancelled(&cancel, async {
        unreachable!("an already-cancelled wait must never start its work")
    })
    .await;
    assert_eq!(outcome, Err::<(), _>(super::AnalysisCancelled));
}

#[tokio::test(start_paused = true)]
async fn until_cancelled_abandons_a_wait_that_is_cancelled_mid_flight() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let probe = cancelled.clone();
    let cancel = move || probe.load(Ordering::SeqCst);
    let flip = async {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        cancelled.store(true, Ordering::SeqCst);
    };
    let wait = super::until_cancelled(&cancel, std::future::pending::<()>());
    let (outcome, ()) = tokio::join!(wait, flip);
    assert_eq!(
        outcome,
        Err(super::AnalysisCancelled),
        "a wait on the webview must end once the scan is cancelled"
    );
}

/// An ad or consent iframe's navigation reaches the same gate as a redirect
/// hop. Before this the analyzer re-navigated to the iframe's document and
/// graded it as the page (an attribute-less `<html>` failing html-has-lang,
/// a 13 ms navigation TTFB for a cached ad helper). The commit tracker tells
/// the two apart, and every payload has to name the document it came from
/// before anything is attached.
#[test]
fn the_analyzer_tracks_main_frame_commits_and_grades_only_admitted_documents() {
    let source = include_str!("analyzer.rs");
    assert!(
        source.contains(".on_page_load("),
        "analyzer must register the page-load handler that records main-frame commits"
    );
    let start = source
        .find("async fn drive_analyzer(")
        .expect("the analyzer must drive its webview through one function");
    let body = &source[start..];
    let body = &body[..body.find("\n}\n").expect("drive_analyzer must end")];
    assert!(
        body.contains("AdmittedDocuments::new(&target)"),
        "the admitted documents start from the analyzed target"
    );
    assert_eq!(
        body.matches("admitted.verify(").count(),
        2,
        "the Core Web Vitals sample and the axe report must each be verified"
    );
    let first_verify = body.find("admitted.verify(").expect("verify present");
    let axe_run = body.find("run_axe_analysis(").expect("axe present");
    assert!(
        first_verify < axe_run,
        "a sample from another document must end the run before the axe budget is spent"
    );
    let observe = body
        .find("admitted.observe_commit(")
        .expect("the committed document must refine the record before anything is verified");
    assert!(
        observe < first_verify,
        "a same-host scheme change is allowed inline and only the commit records it"
    );
}

#[test]
fn a_payload_from_another_document_reports_a_run_that_graded_nothing() {
    let analysis = super::WebviewAnalysis::other_document(
        Some("621.1.15".into()),
        DocumentMismatch::OtherDocument {
            origin: "https://googleads.g.doubleclick.net".into(),
        },
    );
    assert!(
        analysis.browser_ran,
        "the browser did run; it graded nothing"
    );
    assert!(analysis.cwv.is_none());
    assert!(analysis.accessibility.is_none());
    assert_eq!(analysis.browser_build.as_deref(), Some("621.1.15"));
    assert_eq!(
        analysis.error.as_deref(),
        Some("analyzer graded a different document (https://googleads.g.doubleclick.net)")
    );
}

#[test]
fn the_analyzer_closes_its_webview_on_exactly_one_path() {
    let source = include_str!("analyzer.rs");
    assert_eq!(
        source.matches("webview.close()").count(),
        1,
        "cancellation and every failure must share one close, so no analyzer window survives a cancelled scan"
    );
}

#[test]
fn every_wait_while_the_analyzer_holds_a_webview_observes_cancellation() {
    let source = include_str!("analyzer.rs");
    let start = source
        .find("async fn drive_analyzer(")
        .expect("the analyzer must drive its webview through one function");
    let body = &source[start..];
    let body = &body[..body.find("\n}\n").expect("drive_analyzer must end")];
    assert_eq!(
        body.matches(".await").count(),
        body.matches("until_cancelled(").count(),
        "every await taken while the analyzer holds a webview must go through until_cancelled"
    );
    assert!(
        body.matches("until_cancelled(").count() >= 4,
        "the page load, browser build, Core Web Vitals, and axe waits must each be cancellable"
    );
}
