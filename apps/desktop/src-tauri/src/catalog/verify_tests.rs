//! Verification-order and compatibility tests.
//! Signature tests require a configured key; hashing and version tests always run.

use super::*;

#[test]
fn refuses_every_pack_when_no_signing_key_is_configured() {
    // Fail closed. A build without a catalog key must not treat "nothing to
    // verify against" as "verification passed".
    if CATALOG_PUBLIC_KEY.is_some() {
        return;
    }
    let context = VerificationContext {
        engine_version: "1.4.0",
        active_release_sequence: None,
        active_pack_needs_repair: false,
        expected_content_hash: &sha256_hex(b"{}"),
        manifest_release_sequence: 1,
        manifest_catalog_version: "2026-07-28",
    };
    assert!(matches!(
        verify_pack(b"{}", "untrusted", &context),
        Err(VerifyError::NoSigningKeyConfigured)
    ));
}

// The signature of the first published pack, verbatim: the minisign
// signature file, base64-wrapped whole, exactly as the Tauri signer writes
// it and the packer copies it into the manifest. Public data.
const LIVE_WRAPPED_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTZWFCZTU2ZVc3WHpqYlpTbDh3eXdzQWNsdk1IU0w2Zld2QzE2WWxHTUYyUFA0NUNUWU1ucU80R0toT1NJMnJtaFdBQlU2TmNJNlFzWjl1Vzdnc3JFRkUwVUtWM2gwVFFVPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg1MTg4ODk1CWZpbGU6Y2F0YWxvZy5qc29uCjRtS0U3ZVo2WXNmeUhLV09vR3VLbnNCTkwxODhDcHo0SWRFYnNsMjhkZXBNcTl4Y1RrK2VvbjBzRWZ0VTBiOTZSakZENEdoRmFjdUUyVVYzamxRbENnPT0K";

#[test]
fn decodes_the_signature_format_a_real_pack_carries() {
    assert!(decode_wrapped_signature(LIVE_WRAPPED_SIGNATURE).is_ok());

    assert!(Signature::decode(LIVE_WRAPPED_SIGNATURE).is_err());
}

#[test]
fn signature_decoding_refuses_garbage_at_each_layer() {
    // Not base64 at all.
    assert!(matches!(
        decode_wrapped_signature("not base64!!!"),
        Err(VerifyError::MalformedSignature(_))
    ));
    // Valid base64, but not minisign text inside.
    assert!(matches!(
        decode_wrapped_signature("aGVsbG8gd29ybGQ="),
        Err(VerifyError::MalformedSignature(_))
    ));
}

#[test]
fn rejects_a_pack_above_the_size_limit_before_anything_else() {
    let oversized = vec![b'x'; CATALOG_MAX_PACK_BYTES + 1];
    let context = VerificationContext {
        engine_version: "1.4.0",
        active_release_sequence: None,
        active_pack_needs_repair: false,
        expected_content_hash: "unused",
        manifest_release_sequence: 1,
        manifest_catalog_version: "2026-07-28",
    };
    assert!(matches!(
        verify_pack(&oversized, "unused", &context),
        Err(VerifyError::PackTooLarge { .. })
    ));
}

#[test]
fn hashes_are_lowercase_hex_sha256() {
    // Known-answer test: SHA-256 of the empty input.
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn engine_version_comparison_is_numeric_not_lexical() {
    // The bug this prevents: "1.10.0" < "1.9.0" as strings, which would refuse
    // a pack the engine can actually read.
    assert!(engine_meets_minimum("1.10.0", "1.9.0"));
    assert!(engine_meets_minimum("2.0.0", "1.99.99"));
    assert!(!engine_meets_minimum("1.9.0", "1.10.0"));
}

#[test]
fn engine_version_comparison_accepts_an_exact_match() {
    assert!(engine_meets_minimum("1.4.0", "1.4.0"));
}

#[test]
fn prerelease_builds_count_as_their_release_version() {
    // A release candidate has to be able to read the pack it is testing.
    assert!(engine_meets_minimum("1.5.0-rc.1", "1.5.0"));
    assert!(engine_meets_minimum("1.5.0+build.7", "1.5.0"));
}

#[test]
fn a_malformed_minimum_version_fails_closed() {
    assert!(!engine_meets_minimum("1.0.0", "not-a-version"));
    assert!(!engine_meets_minimum("not-a-version", "1.0.0"));
    assert!(!engine_meets_minimum("1.0.0", "1.0"));
    assert!(!engine_meets_minimum("1.0.0", "1.0.0.1"));
    assert!(!engine_meets_minimum("1.0.0", ""));
}

#[test]
fn rollback_refusal_permits_only_the_equal_sequence_repair() {
    // Strictly lower is always refused, repair state or not.
    assert!(rollback_refused(4, 5, false));
    assert!(rollback_refused(4, 5, true));
    // Equal over a healthy floor-matching pack is replay, refused.
    assert!(rollback_refused(5, 5, false));
    assert!(!rollback_refused(5, 5, true));
    // Newer always passes.
    assert!(!rollback_refused(6, 5, false));
    assert!(!rollback_refused(6, 5, true));
}
