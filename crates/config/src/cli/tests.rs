use std::ffi::OsString;
use std::path::Path;

use clap::Parser;
use clap::error::ErrorKind;

use crate::command::{Command, ConfigError, DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES, TrendRange};

use super::{
    Arguments, PLUGIN_MAX_DOWNLOAD_MB_VARIABLE, build_command, parse_plugin_max_download_bytes,
};

#[test]
fn plugin_max_download_bytes_defaults_when_unset_or_empty() {
    assert_eq!(
        parse_plugin_max_download_bytes(|_| None).expect("default when unset"),
        DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES
    );
    assert_eq!(
        parse_plugin_max_download_bytes(|name| {
            (name == PLUGIN_MAX_DOWNLOAD_MB_VARIABLE).then(|| OsString::from(""))
        })
        .expect("default when empty"),
        DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES
    );
}

#[test]
fn plugin_max_download_bytes_parses_positive_mib_override() {
    let parsed = parse_plugin_max_download_bytes(|name| {
        (name == PLUGIN_MAX_DOWNLOAD_MB_VARIABLE).then(|| OsString::from("128"))
    })
    .expect("valid positive MiB override");
    assert_eq!(parsed, 128 * 1024 * 1024);
}

#[test]
fn plugin_max_download_bytes_rejects_zero_negative_non_numeric_and_overflow() {
    for bad in [
        "0",
        "-1",
        "abc",
        "12.5",
        "8796093022208",
        "10000000000000",
        "18446744073709551615",
    ] {
        let error = parse_plugin_max_download_bytes(|name| {
            (name == PLUGIN_MAX_DOWNLOAD_MB_VARIABLE).then(|| OsString::from(bad))
        })
        .expect_err("invalid override must fail");
        assert!(
            matches!(error, ConfigError::InvalidPluginMaxDownloadMb { ref value } if value == bad),
            "expected InvalidPluginMaxDownloadMb for {bad:?}, got {error}"
        );
    }
}

#[test]
fn plugin_max_download_bytes_accepts_signed_64_bit_boundary() {
    let parsed = parse_plugin_max_download_bytes(|name| {
        (name == PLUGIN_MAX_DOWNLOAD_MB_VARIABLE).then(|| OsString::from("8796093022207"))
    })
    .expect("maximum signed 64-bit MiB boundary must succeed");
    assert_eq!(parsed, 8_796_093_022_207 * 1024 * 1024);
    assert!(parsed <= i64::MAX as u64);
}

#[test]
fn trend_defaults_to_latest_20_commits() {
    let arguments = Arguments::try_parse_from(["codeint", "trend"]).expect("parse trend");
    let command = build_command(Path::new("."), arguments.command).expect("build trend");
    assert!(matches!(
        command,
        Command::Trend {
            range: TrendRange::Latest(20),
            ..
        }
    ));
}

#[test]
fn trend_commits_sets_latest_range_below_at_and_above_default() {
    for count in [5usize, 20, 25] {
        let count_arg = count.to_string();
        let arguments = Arguments::try_parse_from(["codeint", "trend", "--commits", &count_arg])
            .expect("parse trend --commits");
        let command = build_command(Path::new("."), arguments.command).expect("build trend");
        assert!(
            matches!(
                command,
                Command::Trend { range, .. } if range == TrendRange::Latest(count)
            ),
            "expected TrendRange::Latest({count}), got {command:?}"
        );
    }
}

#[test]
fn trend_commits_zero_is_rejected() {
    let error = Arguments::try_parse_from(["codeint", "trend", "--commits", "0"])
        .expect_err("trend --commits 0 must fail");
    assert_eq!(error.kind(), ErrorKind::ValueValidation);
}

#[test]
fn trend_since_and_commits_conflict() {
    let error = Arguments::try_parse_from([
        "codeint",
        "trend",
        "--since",
        "2026-01-01",
        "--commits",
        "5",
    ])
    .expect_err("trend --since with --commits must fail");
    assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
}
