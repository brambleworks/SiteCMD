use super::*;

fn parse(url: &str) -> Url {
    Url::parse(url).expect("test url")
}

#[test]
fn the_target_and_its_admitted_hops_are_the_only_gradable_documents() {
    let mut admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert_eq!(admitted.verify(Some("https://example.com/pricing")), Ok(()));
    assert_eq!(
        admitted.verify(Some("https://www.example.com/")),
        Err(DocumentMismatch::OtherDocument {
            origin: "https://www.example.com".into()
        })
    );

    admitted.admit(&parse("https://www.example.com/"));
    assert_eq!(admitted.verify(Some("https://www.example.com/")), Ok(()));
}

#[test]
fn a_subframe_document_is_named_by_its_origin() {
    // The failure the check exists for: an ad iframe's helper page graded
    // as the site.
    let admitted = AdmittedDocuments::new(&parse("https://visityourteam.com/"));
    let mismatch = admitted
        .verify(Some(
            "https://googleads.g.doubleclick.net/pagead/html/r20260901/r20190131/zrt_lookup_fy2021.html",
        ))
        .unwrap_err();
    assert_eq!(
        mismatch.to_string(),
        "analyzer graded a different document (https://googleads.g.doubleclick.net)"
    );
}

#[test]
fn same_host_scheme_and_port_changes_are_the_same_document() {
    // The gate admits these inline, so they never register as hops.
    let admitted = AdmittedDocuments::new(&parse("http://example.com/"));
    assert_eq!(admitted.verify(Some("https://example.com/")), Ok(()));
    assert_eq!(admitted.verify(Some("https://EXAMPLE.com.:8443/")), Ok(()));
}

#[test]
fn a_transport_downgrade_is_not_the_document_that_was_admitted() {
    // An upgrade is the same page reached more safely; the other direction
    // would let a stripped document grade under an https target's name.
    let admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert_eq!(
        admitted.verify(Some("http://example.com/")),
        Err(DocumentMismatch::OtherDocument {
            origin: "http://example.com".into()
        })
    );
    assert_eq!(admitted.verify(Some("https://example.com:8443/")), Ok(()));

    // A hop the runtime actually followed over http admits http for that host.
    let mut with_plain_hop = AdmittedDocuments::new(&parse("https://example.com/"));
    with_plain_hop.admit(&parse("http://example.com/"));
    assert_eq!(with_plain_hop.verify(Some("http://example.com/")), Ok(()));
}

#[test]
fn a_commit_refines_an_admitted_host_and_never_adds_one() {
    // The gate judges the host, so a same-host https-to-http server redirect
    // is allowed inline and never arrives as a deferred hop. Without the
    // commit the record would still say https-only and the run would report
    // grading a different document.
    let mut admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert!(admitted.verify(Some("http://example.com/")).is_err());
    admitted.observe_commit(&parse("http://example.com/"));
    assert_eq!(admitted.verify(Some("http://example.com/")), Ok(()));

    // A commit on a host the runtime never admitted is the page moving
    // itself, and must not become gradable.
    admitted.observe_commit(&parse(
        "https://googleads.g.doubleclick.net/pagead/html/r.html",
    ));
    assert_eq!(
        admitted.verify(Some(
            "https://googleads.g.doubleclick.net/pagead/html/r.html"
        )),
        Err(DocumentMismatch::OtherDocument {
            origin: "https://googleads.g.doubleclick.net".into()
        })
    );
}

#[test]
fn only_a_web_document_scheme_can_be_admitted() {
    // A non-document URL that happens to carry an admitted host is not the
    // page: nothing reads a payload out of it.
    let admitted = AdmittedDocuments::new(&parse("http://example.com/"));
    assert!(admitted.verify(Some("ws://example.com/socket")).is_err());
    assert!(admitted.verify(Some("ftp://example.com/file")).is_err());
}

#[test]
fn a_payload_is_verified_through_its_own_identity_field() {
    let admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert_eq!(
        admitted.verify_payload(&serde_json::json!({ "document_url": "https://example.com/x" })),
        Ok(())
    );
    assert_eq!(
        admitted.verify_payload(&serde_json::json!({
            "document_url": "https://googleads.g.doubleclick.net/pagead/html/r.html"
        })),
        Err(DocumentMismatch::OtherDocument {
            origin: "https://googleads.g.doubleclick.net".into()
        })
    );
    assert_eq!(
        admitted.verify_payload(&serde_json::json!({ "violations": [] })),
        Err(DocumentMismatch::Unidentified)
    );
}

#[test]
fn a_payload_without_identity_cannot_be_graded() {
    let admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert_eq!(admitted.verify(None), Err(DocumentMismatch::Unidentified));
    assert_eq!(
        admitted.verify(Some("not a url")),
        Err(DocumentMismatch::Unidentified)
    );
    assert_eq!(
        DocumentMismatch::Unidentified.to_string(),
        "analyzer payload did not identify the document it was read from"
    );
}

#[test]
fn the_blank_page_is_never_an_admitted_document() {
    let admitted = AdmittedDocuments::new(&parse("https://example.com/"));
    assert_eq!(
        admitted.verify(Some("about:blank")),
        Err(DocumentMismatch::OtherDocument {
            origin: "null".into()
        })
    );
}

#[test]
fn ip_literal_targets_are_admitted_by_their_literal() {
    let admitted = AdmittedDocuments::new(&parse("http://127.0.0.1:3000/"));
    assert_eq!(admitted.verify(Some("http://127.0.0.1:3000/app")), Ok(()));
    assert!(admitted.verify(Some("http://localhost:3000/")).is_err());
}

#[test]
fn payload_document_url_reads_only_the_recorded_string() {
    assert_eq!(
        payload_document_url(&serde_json::json!({
            "document_url": "https://example.com/",
            "violations": []
        }))
        .as_deref(),
        Some("https://example.com/")
    );
    assert_eq!(
        payload_document_url(&serde_json::json!({ "violations": [] })),
        None
    );
    assert_eq!(
        payload_document_url(&serde_json::json!({ "document_url": null })),
        None
    );
    assert_eq!(payload_document_url(&serde_json::json!("string")), None);
}
