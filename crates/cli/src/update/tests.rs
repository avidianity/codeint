use std::ffi::OsString;
use std::fs;
use std::path::Path;

use codeint_config::ConfigFileKind;
use tempfile::TempDir;

use super::{
    CURL_CONNECT_TIMEOUT_SECONDS, CURL_DOWNLOAD_MAX_TIME_SECONDS, CURL_METADATA_MAX_TIME_SECONDS,
    UpdateError, build_curl_download_args, build_curl_text_args, constant_time_equal,
    extract_tag_name, format_schema_sync_notice, parse_sha256_hex, verify_archive_sha256,
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

    assert!(has_flag_pair(&args, "--proto", "=https"));
    assert!(args.iter().any(|arg| arg == "--tlsv1.2"));
    assert!(has_flag_pair(&args, "--proto-redir", "=https"));
    assert!(has_flag_pair(
        &args,
        "--connect-timeout",
        CURL_CONNECT_TIMEOUT_SECONDS
    ));
    assert!(has_flag_pair(
        &args,
        "--max-time",
        CURL_METADATA_MAX_TIME_SECONDS
    ));
    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(args[args.len() - 1], url);
}

#[test]
fn build_curl_download_args_enforces_https_tls12_timeouts_and_url_terminator() {
    let url = "https://github.com/avidianity/codeint/releases/download/v1.0.0/codeint.tar.gz";
    let dest = Path::new("/tmp/codeint.tar.gz");
    let args = as_strings(&build_curl_download_args(url, dest));

    assert!(has_flag_pair(&args, "--proto", "=https"));
    assert!(args.iter().any(|arg| arg == "--tlsv1.2"));
    assert!(has_flag_pair(&args, "--proto-redir", "=https"));
    assert!(has_flag_pair(
        &args,
        "--connect-timeout",
        CURL_CONNECT_TIMEOUT_SECONDS
    ));
    assert!(has_flag_pair(
        &args,
        "--max-time",
        CURL_DOWNLOAD_MAX_TIME_SECONDS
    ));
    assert!(has_flag_pair(&args, "--output", "/tmp/codeint.tar.gz"));
    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(args[args.len() - 1], url);
}

#[test]
fn extract_tag_name_parses_json_and_rejects_malformed_or_missing_tags() {
    let with_body_decoy =
        r#"{"body":"mentions \"tag_name\": \"v0.0.1\"","tag_name":"1.2.3","draft":false}"#;
    assert_eq!(
        extract_tag_name(with_body_decoy).expect("valid release json"),
        "v1.2.3"
    );

    assert!(matches!(
        extract_tag_name(r#"{"tag_name":"v1.2.3""#),
        Err(UpdateError::LatestRelease { .. })
    ));
    assert!(matches!(
        extract_tag_name(r#"{"name":"v1.2.3"}"#),
        Err(UpdateError::LatestRelease { .. })
    ));
    assert!(matches!(
        extract_tag_name(r#"{"tag_name":123}"#),
        Err(UpdateError::LatestRelease { .. })
    ));
    assert!(matches!(
        extract_tag_name(r#"{"tag_name":"   "}"#),
        Err(UpdateError::LatestRelease { .. })
    ));
}

#[test]
fn verify_archive_sha256_accepts_matching_digest_and_rejects_mismatch_or_malformed_hex() {
    let temp = TempDir::new().expect("temp dir");
    let archive_path = temp.path().join("codeint.tar.gz");
    fs::write(&archive_path, b"abc").expect("write archive");
    let expected_abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    verify_archive_sha256(&archive_path, "codeint.tar.gz", expected_abc)
        .expect("matching lowercase digest");
    verify_archive_sha256(
        &archive_path,
        "codeint.tar.gz",
        &expected_abc.to_ascii_uppercase(),
    )
    .expect("matching uppercase digest");

    let wrong_digest = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(matches!(
        verify_archive_sha256(&archive_path, "codeint.tar.gz", wrong_digest),
        Err(UpdateError::ChecksumMismatch { .. })
    ));
    assert!(matches!(
        verify_archive_sha256(&archive_path, "codeint.tar.gz", "not-a-sha256"),
        Err(UpdateError::ChecksumMismatch { .. })
    ));
}

#[test]
fn constant_time_equal_and_hex_decoder_validate_digests() {
    let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let digest = parse_sha256_hex(hex).expect("valid 64-char hex");
    assert!(constant_time_equal(&digest, &digest));

    let mut flipped = digest;
    flipped[31] ^= 0x01;
    assert!(!constant_time_equal(&digest, &flipped));
    assert!(parse_sha256_hex("short").is_none());
    assert!(
        parse_sha256_hex("zz7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .is_none()
    );
}

#[test]
fn format_schema_sync_notice_matches_each_config_syntax() {
    let url =
        "https://raw.githubusercontent.com/avidianity/codeint/v1.0.0/schema/codeint.schema.json";

    assert_eq!(
        format_schema_sync_notice(ConfigFileKind::Json, Path::new("codeint.json"), url),
        format!("Updated $schema in codeint.json to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(ConfigFileKind::Jsonc, Path::new("codeint.jsonc"), url),
        format!("Updated $schema in codeint.jsonc to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(ConfigFileKind::Toml, Path::new("codeint.toml"), url),
        format!("Updated #:schema directive in codeint.toml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(ConfigFileKind::Yaml, Path::new("codeint.yaml"), url),
        format!("Updated # yaml-language-server: $schema= directive in codeint.yaml to {url}")
    );
    assert_eq!(
        format_schema_sync_notice(
            ConfigFileKind::DotCodeintYml,
            Path::new(".codeint/config.yml"),
            url
        ),
        format!(
            "Updated # yaml-language-server: $schema= directive in .codeint/config.yml to {url}"
        )
    );
}
