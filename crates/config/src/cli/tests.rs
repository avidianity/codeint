use std::ffi::OsString;

use crate::command::{ConfigError, DEFAULT_PLUGIN_MAX_DOWNLOAD_BYTES};

use super::{PLUGIN_MAX_DOWNLOAD_MB_VARIABLE, parse_plugin_max_download_bytes};

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
    for bad in ["0", "-1", "abc", "12.5", "18446744073709551615"] {
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
