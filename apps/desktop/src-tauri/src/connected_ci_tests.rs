//! Connected CI wire-contract tests.

use super::*;

use sitecmd_engine::coverage::ScanCoverageKind;
use sitecmd_engine::sync::{
    CodeBasis, CodeBasisKind, CodeOccurrence, CodeProvenance, CodeVersions, DesktopProvenanceKind,
    WireCoverage, WireExecutionProfile,
};
use sitecmd_engine::vocab::{IssueConfidence, Severity};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SITE_ID: &str = "site_ci_door";
const LOCATION_HASH: &str = "8c1f00000000000000000000000000000000000000000000000000000000abcd";

fn snapshot() -> CodeSnapshot {
    CodeSnapshot {
        observed_at: 1_754_784_000_000,
        based_on_event_sequence: 0,
        versions: CodeVersions {
            engine_release: "1.5.4".into(),
            fingerprint_schema: 1,
            fingerprint_key_version: 1,
            canonicalizer: 1,
        },
        manifest_digest: "9e4b0000".into(),
        evaluation_time: 1_754_783_900_000,
        execution_profile: WireExecutionProfile::default(),
        key_commitment: "0f".repeat(32),
        code_basis: CodeBasis {
            commit_sha: Some("c0ffee1".into()),
            kind: CodeBasisKind::ExactCheckout,
            unvouched: Vec::new(),
        },
        coverage: WireCoverage {
            kind: ScanCoverageKind::Project,
            complete: true,
            routes: Vec::new(),
            checks: vec!["code_scan.security".into()],
            exceptions: Vec::new(),
        },
        occurrences: vec![CodeOccurrence {
            check: "code_scan.security".into(),
            location_hash: LOCATION_HASH.into(),
            instance_count: 1,
            severity: Severity::High,
            confidence: Some(IssueConfidence::NeedsReview),
            provenance: CodeProvenance {
                commit_sha: Some("c0ffee1".into()),
                kind: DesktopProvenanceKind::Unknown,
            },
        }],
    }
}

fn facts() -> DeploymentFacts {
    DeploymentFacts {
        provider_deployment_id: "run-42".into(),
        commit_sha: "c0ffee1".into(),
        ..DeploymentFacts::default()
    }
}

#[test]
fn the_submission_is_the_ci_shape_and_carries_no_producer_sequence() {
    // CI orders by deployment and retries by idempotency key, not sequence.
    let rendered = ci_submission_body(SITE_ID, &snapshot(), &facts()).expect("render");
    let body: Value = serde_json::from_str(&rendered).expect("body JSON");

    let mut members: Vec<&str> = body
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(
        members,
        ["deployment", "schema_version", "site_id", "snapshot"]
    );
    assert_eq!(body["site_id"], SITE_ID);
    assert_eq!(body["schema_version"], 1);
    assert_eq!(body["deployment"]["provider_deployment_id"], "run-42");
    assert_eq!(body["deployment"]["commit_sha"], "c0ffee1");
    // The snapshot is the single `snapshot` member, not the desktop's
    // `snapshots.web`/`snapshots.code` envelope.
    assert!(body.get("snapshots").is_none());
    assert_eq!(body["snapshot"]["code_basis"]["kind"], "exact_checkout");
    assert_eq!(
        body["snapshot"]["occurrences"][0]["location_hash"],
        LOCATION_HASH
    );
}

#[test]
fn the_client_never_states_a_provenance_the_server_assigns() {
    let rendered = ci_submission_body(SITE_ID, &snapshot(), &facts()).expect("render");
    let body: Value = serde_json::from_str(&rendered).expect("body JSON");

    let occurrence = &body["snapshot"]["occurrences"][0];
    assert!(occurrence.get("provenance").is_none(), "{occurrence}");
    assert!(!rendered.contains("provenance"));

    // The snapshot-level basis is a different fact and does travel: it
    // describes the checkout, and the attested door reads it.
    assert!(body["snapshot"].get("code_basis").is_some());
}

#[test]
fn the_snapshot_carries_no_desktop_event_watermark() {
    let rendered = ci_submission_body(SITE_ID, &snapshot(), &facts()).expect("render");
    let body: Value = serde_json::from_str(&rendered).expect("body JSON");
    assert!(body["snapshot"].get("based_on_event_sequence").is_none());
    assert!(body["snapshot"].get("observed_at").is_some());
}

#[test]
fn a_resent_submission_replays_and_a_new_one_does_not_collide() {
    let first = ci_submission_body(SITE_ID, &snapshot(), &facts()).expect("render");
    let resent = ci_submission_body(SITE_ID, &snapshot(), &facts()).expect("render");
    assert_eq!(ci_idempotency_key(&first), ci_idempotency_key(&resent));

    let later = ci_submission_body(
        SITE_ID,
        &CodeSnapshot {
            observed_at: 1_754_784_999_000,
            ..snapshot()
        },
        &facts(),
    )
    .expect("render");
    assert_ne!(ci_idempotency_key(&first), ci_idempotency_key(&later));

    // Within the header bound the openapi document states (128), even though
    // the handler's own identifier check would admit four times that.
    let key = ci_idempotency_key(&first);
    assert_eq!(key.len(), 64);
    assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn each_door_is_addressed_under_the_site_the_token_is_bound_to() {
    let client = ConnectedServiceClient::for_test_endpoint(
        "https://connect.sitecmd.com",
        "sitecmd_ci_do_not_log",
    )
    .expect("test client");

    assert_eq!(
        client
            .url(&ci_sync_path(SITE_ID))
            .expect("sync url")
            .as_str(),
        "https://connect.sitecmd.com/v1/sites/site_ci_door/sync/ci"
    );
    assert_eq!(
        client
            .url(&ci_deployments_path(SITE_ID))
            .expect("deployments url")
            .as_str(),
        "https://connect.sitecmd.com/v1/sites/site_ci_door/deployments"
    );
}

#[test]
fn github_actions_requires_the_oidc_permission_and_requests_the_connect_audience() {
    let values = HashMap::from([
        ("GITHUB_ACTIONS", "true"),
        (
            "ACTIONS_ID_TOKEN_REQUEST_URL",
            "https://pipelines.actions.githubusercontent.com/token?api-version=2.0&audience=wrong",
        ),
        ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "runner-bearer"),
    ]);
    let request = github_actions_oidc_request(
        CiSubmissionAttestation::GithubOidc,
        |name| values.get(name).map(|value| (*value).to_string()),
        "https://connect.sitecmd.com",
    )
    .expect("complete GitHub Actions environment")
    .expect("GitHub Actions requests a witness");

    assert_eq!(request.bearer.as_str(), "runner-bearer");
    assert_eq!(
        request.url.as_str(),
        "https://pipelines.actions.githubusercontent.com/token?api-version=2.0&audience=https%3A%2F%2Fconnect.sitecmd.com"
    );

    let missing_permission = HashMap::from([("GITHUB_ACTIONS", "true")]);
    let missing = github_actions_oidc_request(
        CiSubmissionAttestation::GithubOidc,
        |name| {
            missing_permission
                .get(name)
                .map(|value| (*value).to_string())
        },
        "https://connect.sitecmd.com",
    );
    let Err(error) = missing else {
        panic!("Actions without id-token: write must fail visibly");
    };
    assert!(error.contains("id-token: write"));
}

#[test]
fn a_workflow_pinned_credential_on_another_runner_mints_no_witness() {
    let elsewhere = HashMap::from([("CI", "true")]);
    let witness = github_actions_oidc_request(
        CiSubmissionAttestation::GithubOidc,
        |name| elsewhere.get(name).map(|value| (*value).to_string()),
        "https://connect.sitecmd.com",
    )
    .expect("a non-GitHub runner is not an error here");
    assert!(witness.is_none());
}

#[test]
fn generic_ci_skips_github_oidc_with_or_without_the_id_token_permission() {
    let missing_permission = HashMap::from([("GITHUB_ACTIONS", "true")]);
    let without_permission = github_actions_oidc_request(
        CiSubmissionAttestation::Unattested,
        |name| {
            missing_permission
                .get(name)
                .map(|value| (*value).to_string())
        },
        "https://connect.sitecmd.com",
    )
    .expect("generic CI does not require an OIDC permission");
    assert!(without_permission.is_none());

    let complete_environment = HashMap::from([
        ("GITHUB_ACTIONS", "true"),
        (
            "ACTIONS_ID_TOKEN_REQUEST_URL",
            "https://pipelines.actions.githubusercontent.com/token",
        ),
        ("ACTIONS_ID_TOKEN_REQUEST_TOKEN", "runner-bearer"),
    ]);
    let with_permission = github_actions_oidc_request(
        CiSubmissionAttestation::Unattested,
        |name| {
            complete_environment
                .get(name)
                .map(|value| (*value).to_string())
        },
        "https://connect.sitecmd.com",
    )
    .expect("generic CI ignores an unrelated OIDC permission");
    assert!(with_permission.is_none());
}

#[tokio::test]
async fn submission_sends_the_github_oidc_witness_in_its_own_sensitive_header() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let captured = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut bytes = vec![0_u8; 32 * 1024];
        let read = stream.read(&mut bytes).await.expect("read request");
        let request = String::from_utf8_lossy(&bytes[..read]).to_string();
        let body = r#"{"event_sequence":1,"status":"applied"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("respond");
        request
    });
    let client = ConnectedServiceClient::for_test_endpoint(
        &format!("http://{address}"),
        "sitecmd_ci_do_not_log",
    )
    .expect("test client");

    client
        .submit_ci_evidence(SITE_ID, "{}", Some("signed.github.oidc"))
        .await
        .expect("submission accepted");
    let request = captured.await.expect("capture task").to_ascii_lowercase();
    assert!(request.contains("x-github-oidc-token: signed.github.oidc\r\n"));
}

#[test]
fn deployment_facts_omit_what_the_caller_did_not_state() {
    let body = serde_json::to_value(facts()).expect("encode");
    let mut members: Vec<&str> = body
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(members, ["commit_sha", "provider_deployment_id"]);

    let full = serde_json::to_value(DeploymentFacts {
        git_ref: Some("refs/heads/main".into()),
        previous_sha: Some("beef123".into()),
        target: Some("production".into()),
        provider_created_at: Some("2026-08-09T00:00:00.000Z".into()),
        ..facts()
    })
    .expect("encode");
    // The wire name is `ref`, which cannot be a Rust field name.
    assert_eq!(full["ref"], "refs/heads/main");
    assert_eq!(full["target"], "production");
    assert_eq!(full["provider_created_at"], "2026-08-09T00:00:00.000Z");
}

#[test]
fn deployment_facts_are_refused_here_with_the_flag_that_names_them() {
    // The service answers a bad SHA with a bare malformed_request that names
    // no field, and a runner reading a red build cannot act on that.
    assert!(facts().validate().is_ok());

    let error = DeploymentFacts {
        commit_sha: "nothex!".into(),
        ..facts()
    }
    .validate()
    .expect_err("non-hex sha");
    assert!(error.contains("--commit"), "{error}");

    let error = DeploymentFacts {
        commit_sha: "C0FFEE1".into(),
        ..facts()
    }
    .validate()
    .expect_err("uppercase sha");
    assert!(error.contains("lowercase"), "{error}");

    let error = DeploymentFacts {
        provider_deployment_id: String::new(),
        ..facts()
    }
    .validate()
    .expect_err("empty identity");
    assert!(error.contains("--deployment-id"), "{error}");

    let error = DeploymentFacts {
        target: Some(String::new()),
        ..facts()
    }
    .validate()
    .expect_err("empty target");
    assert!(error.contains("--target"), "{error}");

    let error = DeploymentFacts {
        git_ref: Some("r".repeat(257)),
        ..facts()
    }
    .validate()
    .expect_err("oversized ref");
    assert!(error.contains("--ref"), "{error}");

    let error = DeploymentFacts {
        published: true,
        ..facts()
    }
    .validate()
    .expect_err("published without ordering");
    assert!(error.contains("--ordering-authority"), "{error}");

    let ordered = DeploymentFacts {
        published: true,
        ordering: Some(PublishOrdering {
            kind: "publish_sequence".into(),
            authority_id: "github:octo/app:deploy".into(),
            epoch: 2,
            publish_sequence: Some(17),
            predecessor_deployment_id: None,
        }),
        ..facts()
    };
    ordered.validate().expect("ordered publication");
    let value = serde_json::to_value(ordered).expect("wire JSON");
    assert_eq!(value["published"], true);
    assert_eq!(value["ordering"]["kind"], "publish_sequence");
    assert_eq!(value["ordering"]["publish_sequence"], 17);
}

#[test]
fn both_doors_report_creation_though_they_place_it_differently() {
    let deployment: DeploymentReceipt = serde_json::from_str(
        r#"{"created": true, "deployment": {"provider": "ci",
            "provider_deployment_id": "run-42", "commit_sha": "c0ffee1",
            "ref": null, "previous_sha": null, "target": null,
            "ordering": "unknown", "provider_created_at": null,
            "received_at": "2026-08-10T00:00:00.000Z"}}"#,
    )
    .expect("deployments answer");
    assert!(deployment.created);
    assert_eq!(deployment.deployment.ordering, "unknown");
    assert_eq!(deployment.deployment.created, None);

    let submission: CiSubmissionReceipt = serde_json::from_str(
        r#"{"status": "applied", "event_sequence": 14,
            "deployment": {"provider": "ci", "provider_deployment_id": "run-42",
            "commit_sha": "c0ffee1", "ordering": "creation_sequence",
            "received_at": "2026-08-10T00:00:00.000Z", "created": false}}"#,
    )
    .expect("submission answer");
    assert_eq!(submission.status.as_deref(), Some("applied"));
    assert_eq!(submission.event_sequence, Some(14));
    assert!(!submission.created_deployment());
}

#[test]
fn a_published_record_is_read_as_the_integer_column_the_service_answers_with() {
    let receipt: CiSubmissionReceipt = serde_json::from_str(
        r#"{"status": "applied", "event_sequence": 21, "provenance": "exact",
            "state_revision": 8, "canonical_snapshot_id": "snap_7",
            "deployment": {"provider": "ci", "provider_deployment_id": "run-42",
            "commit_sha": "c0ffee1", "ref": "refs/heads/main", "previous_sha": null,
            "target": "production", "ordering": "publish_sequence",
            "provider_created_at": null, "received_at": "2026-08-10T00:00:00.000Z",
            "published": 1, "authority_kind": "publish_attestation",
            "authority_id": "github:1296269:authority", "authority_epoch": 3,
            "publish_sequence": 8, "predecessor_deployment_id": null,
            "became_current_at": "2026-08-10T00:00:00.000Z", "superseded_at": null,
            "immutable_facts_hash": "9e4b", "created": true}}"#,
    )
    .expect("a governing submission answer");

    let deployment = receipt.deployment.expect("deployment record");
    assert_eq!(deployment.published, 1);
    assert_eq!(deployment.ordering, "publish_sequence");
    assert_eq!(deployment.authority_epoch, Some(3));
    assert_eq!(deployment.publish_sequence, Some(8));
    assert_eq!(deployment.immutable_facts_hash.as_deref(), Some("9e4b"));
    // Members this build has never heard of do not refuse a receipt: the door
    // is free to say more than the client reads.
    assert_eq!(deployment.created, Some(true));
}

#[test]
fn an_answer_that_states_no_sequence_is_not_read_as_the_genesis_one() {
    let receipt: CiSubmissionReceipt = serde_json::from_str(
        r#"{"status": "noncanonical_snapshot", "canonical_snapshot_id": "snap_7"}"#,
    )
    .expect("noncanonical answer");
    assert_eq!(receipt.status.as_deref(), Some("noncanonical_snapshot"));
    assert_eq!(receipt.event_sequence, None);
    assert!(receipt.deployment.is_none());
    assert!(!receipt.created_deployment());
}
