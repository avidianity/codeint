use std::error::Error;
use std::ffi::OsString;
use std::path::Path;

use codeint_config::{
    DEFAULT_UPDATE_CONNECT_TIMEOUT_SECONDS, DEFAULT_UPDATE_DOWNLOAD_MAX_TIME_SECONDS,
    DEFAULT_UPDATE_METADATA_MAX_TIME_SECONDS, RewrittenSchemaForm, Sha256Digest,
};

use super::{
    UpdateError, build_curl_download_args, build_curl_text_args, constant_time_equal,
    extract_tag_name, format_schema_sync_notice, validate_release_tag, verify_sha256_bytes,
};

fn as_strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn has_flag_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
}

#[test]
fn build_curl_text_args_enforces_https_tls12_timeouts_and_url_terminator() {
    let url = "-https://example.invalid/releases/latest";
    let args = as_strings(&build_curl_text_args(url));
    let connect_timeout = DEFAULT_UPDATE_CONNECT_TIMEOUT_SECONDS.to_string();
    let metadata_max_time = DEFAULT_UPDATE_METADATA_MAX_TIME_SECONDS.to_string();

    assert!(has_flag_pair(&args, "--proto", "=https"));
    assert!(args.iter().any(|arg| arg == "--tlsv1.2"));
    assert!(has_flag_pair(&args, "--proto-redir", "=https"));
    assert!(has_flag_pair(&args, "--connect-timeout", &connect_timeout));
    assert!(has_flag_pair(&args, "--max-time", &metadata_max_time));
    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(args[args.len() - 1], url);
}

#[test]
fn build_curl_download_args_enforces_https_tls12_timeouts_and_url_terminator() {
    let url = "https://github.com/avidianity/codeint/releases/download/v1.0.0/codeint.tar.gz";
    let dest = Path::new("/tmp/codeint.tar.gz");
    let args = as_strings(&build_curl_download_args(url, dest));
    let connect_timeout = DEFAULT_UPDATE_CONNECT_TIMEOUT_SECONDS.to_string();
    let download_max_time = DEFAULT_UPDATE_DOWNLOAD_MAX_TIME_SECONDS.to_string();

    assert!(has_flag_pair(&args, "--proto", "=https"));
    assert!(args.iter().any(|arg| arg == "--tlsv1.2"));
    assert!(has_flag_pair(&args, "--proto-redir", "=https"));
    assert!(has_flag_pair(&args, "--connect-timeout", &connect_timeout));
    assert!(has_flag_pair(&args, "--max-time", &download_max_time));
    assert!(has_flag_pair(&args, "--output", "/tmp/codeint.tar.gz"));
    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(args[args.len() - 1], url);
}

#[test]
fn extract_tag_name_parses_semver_json_and_rejects_malformed_or_traversal_tags() {
    let with_body_decoy =
        r#"{"body":"mentions \"tag_name\": \"v0.0.1\"","tag_name":"1.2.3","draft":false}"#;
    assert_eq!(
        extract_tag_name(with_body_decoy).expect("valid release json"),
        "v1.2.3"
    );
    assert_eq!(
        extract_tag_name(r#"{"tag_name":"v1.2.3-rc.1"}"#).expect("valid prerelease tag"),
        "v1.2.3-rc.1"
    );

    for malformed in [
        r#"{"tag_name":"v1.2.3""#,
        r#"{"name":"v1.2.3"}"#,
        r#"{"tag_name":123}"#,
    ] {
        let error = extract_tag_name(malformed).expect_err("malformed json must fail");
        assert!(matches!(error, UpdateError::LatestReleaseJson(_)));
        assert!(error.source().is_some());
    }

    for invalid_tag in [
        "",
        "   ",
        "v",
        "vv1.2.3",
        "v1.2",
        "v1.2.3.4",
        "v01.2.3",
        "v1.2.3-",
        "v1.2.3-rc..1",
        "v1.2.3-.rc",
        "v1.2.3-rc.",
        "v1/../../../../attacker/repo/releases/download/v1",
        "v1.2.3/../v0.0.1",
        "v1.2.3\\evil",
        "v1.2.3?token=abc",
        "v1.2.3#fragment",
        "v1.2.3 extra",
    ] {
        assert!(
            matches!(
                validate_release_tag(invalid_tag),
                Err(UpdateError::LatestRelease { .. })
            ),
            "expected {invalid_tag:?} to be rejected"
        );
        let payload = serde_json::json!({ "tag_name": invalid_tag }).to_string();
        assert!(
            matches!(
                extract_tag_name(&payload),
                Err(UpdateError::LatestRelease { .. })
            ),
            "expected JSON tag_name {invalid_tag:?} to be rejected"
        );
    }
}

#[test]
fn verify_sha256_bytes_accepts_matching_digest_and_rejects_mismatch_or_malformed_hex() {
    let expected_abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    verify_sha256_bytes(b"abc", "codeint.tar.gz", expected_abc).expect("matching lowercase digest");
    verify_sha256_bytes(b"abc", "codeint.tar.gz", &expected_abc.to_ascii_uppercase())
        .expect("matching uppercase digest");

    let wrong_digest = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(matches!(
        verify_sha256_bytes(b"abc", "codeint.tar.gz", wrong_digest),
        Err(UpdateError::ChecksumMismatch { .. })
    ));
    assert!(matches!(
        verify_sha256_bytes(b"abc", "codeint.tar.gz", "not-a-sha256"),
        Err(UpdateError::ChecksumMismatch { .. })
    ));
}

#[test]
fn constant_time_equal_compares_parsed_sha256_digests() {
    let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let digest = Sha256Digest::parse_hex(hex).expect("valid 64-char hex");
    assert!(constant_time_equal(digest.as_bytes(), digest.as_bytes()));

    let mut flipped = *digest.as_bytes();
    flipped[31] ^= 0x01;
    assert!(!constant_time_equal(digest.as_bytes(), &flipped));
}

#[test]
fn format_schema_sync_notice_matches_each_rewritten_schema_form() {
    let url =
        "https://raw.githubusercontent.com/avidianity/codeint/v1.0.0/schema/codeint.schema.json";

    assert_eq!(
        format_schema_sync_notice(RewrittenSchemaForm::JsonKey, Path::new("codeint.json"), url),
        format!("Updated $schema in codeint.json to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            RewrittenSchemaForm::TomlDirective,
            Path::new("codeint.toml"),
            url
        ),
        format!("Updated #:schema directive in codeint.toml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(RewrittenSchemaForm::TomlKey, Path::new("codeint.toml"), url),
        format!("Updated $schema in codeint.toml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            RewrittenSchemaForm::TomlDirectiveAndKey,
            Path::new("codeint.toml"),
            url
        ),
        format!("Updated #:schema directive and $schema in codeint.toml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            RewrittenSchemaForm::YamlDirective,
            Path::new("codeint.yaml"),
            url
        ),
        format!("Updated # yaml-language-server: $schema= directive in codeint.yaml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            RewrittenSchemaForm::YamlKey,
            Path::new(".codeint/config.yml"),
            url
        ),
        format!("Updated $schema in .codeint/config.yml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            RewrittenSchemaForm::YamlDirectiveAndKey,
            Path::new("codeint.yaml"),
            url
        ),
        format!(
            "Updated # yaml-language-server: $schema= directive and $schema in codeint.yaml to {url}"
        )
    );
}
