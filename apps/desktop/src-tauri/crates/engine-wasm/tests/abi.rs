//! Native tests for the exported allocation, scoring, framing, and free ABI.

use sitecmd_engine::scoring::calculator::ScoreSnapshot;
use sitecmd_engine_wasm::{scorer_alloc, scorer_free, scorer_score};

const CORPUS: &str = include_str!("../../engine/fixtures/score/golden.json");

fn call_through_abi(request: &[u8]) -> Vec<u8> {
    unsafe {
        let input = scorer_alloc(request.len() as u32);
        core::ptr::copy_nonoverlapping(request.as_ptr(), input, request.len());
        let frame = scorer_score(input, request.len() as u32) as *mut u8;
        let mut length_bytes = [0u8; 4];
        core::ptr::copy_nonoverlapping(frame, length_bytes.as_mut_ptr(), 4);
        let payload_length = u32::from_le_bytes(length_bytes) as usize;
        let payload = core::slice::from_raw_parts(frame.add(4), payload_length).to_vec();
        scorer_free(frame, (4 + payload_length) as u32);
        payload
    }
}

#[derive(serde::Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    now_ms: i64,
    groups: serde_json::Value,
    expected: ScoreSnapshot,
}

#[test]
fn golden_corpus_round_trips_the_abi() {
    let corpus: Corpus = serde_json::from_str(CORPUS).expect("golden.json parses");
    assert!(!corpus.cases.is_empty());
    for case in &corpus.cases {
        let request = serde_json::json!({ "groups": case.groups, "now_ms": case.now_ms });
        let payload = call_through_abi(&serde_json::to_vec(&request).expect("request serializes"));
        let snapshot: ScoreSnapshot = serde_json::from_slice(&payload)
            .unwrap_or_else(|error| panic!("{}: payload is a snapshot, not {error}", case.name));
        assert_eq!(snapshot.overall, case.expected.overall, "{}", case.name);
        assert_eq!(
            (snapshot.critical_count, snapshot.high_count),
            (case.expected.critical_count, case.expected.high_count),
            "{}",
            case.name
        );
        assert_eq!(
            snapshot.exploitable_capped, case.expected.exploitable_capped,
            "{}",
            case.name
        );
    }
}

#[test]
fn malformed_request_returns_an_error_payload() {
    let payload = call_through_abi(b"{ not json");
    let value: serde_json::Value = serde_json::from_slice(&payload).expect("error frame is JSON");
    let message = value["error"].as_str().expect("error carries a message");
    assert!(!message.is_empty());
}

#[test]
fn identical_requests_produce_identical_payloads() {
    let request = br#"{"groups":[{"check_id":"web.security.csp","category":"security","severity":"critical","status":"new","members":[{"severity":"critical","confidence":"confirmed"}]}],"now_ms":0}"#;
    assert_eq!(call_through_abi(request), call_through_abi(request));
}
