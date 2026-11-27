use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use codeint_config::{ConfigFileKind, ConfigSyntax};
use codeint_plugin_host::{sha256, sha256_hex};
use serde::Deserialize;

#[cfg(test)]
mod tests;

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/avidianity/codeint/releases/latest";
const DEFAULT_RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/avidianity/codeint/releases/download";
const CURL_CONNECT_TIMEOUT_SECONDS: &str = "15";
const CURL_METADATA_MAX_TIME_SECONDS: &str = "30";
const CURL_DOWNLOAD_MAX_TIME_SECONDS: &str = "300";

#[derive(Deserialize)]
struct LatestReleaseResponse {
    tag_name: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum UpdateError {
    #[error("cannot locate the running codeint executable")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("codeint at {} is managed by a package manager", path.display())]
    ManagedByPackageManager { path: PathBuf },
    #[error("unsupported platform for self-update")]
    UnsupportedPlatform,
    #[error("cannot query the latest release: {detail}")]
    LatestRelease { detail: String },
    #[error("cannot download {url}: {detail}")]
    Download { url: String, detail: String },
    #[error("checksum verification failed for {file}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },
    #[error("no SHA-256 checksum entry found for {file} in SHA256SUMS")]
    MissingChecksum { file: String },
    #[error("cannot compute SHA-256 checksum: {detail}")]
    ChecksumTool { detail: String },
    #[error("cannot extract release archive: {detail}")]
    Extract { detail: String },
    #[error("cannot replace executable at {}", path.display())]
    Replace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn update(
    check_only: bool,
    requested_version: Option<&str>,
    release_base_url: Option<&str>,
) -> Result<String, UpdateError> {
    let exe_path = std::env::current_exe().map_err(UpdateError::CurrentExecutable)?;
    if is_node_modules_install(&exe_path) {
        return Err(UpdateError::ManagedByPackageManager { path: exe_path });
    }
    let current = env!("CARGO_PKG_VERSION");
    let current_tag = format!("v{current}");
    let target_tag = match requested_version {
        Some(tag) => normalize_tag(tag),
        None => resolve_latest_tag(release_base_url)?,
    };
    if target_tag == current_tag && (requested_version.is_none() || check_only) {
        let base = format!("codeint {current} is already up to date ({target_tag})");
        return Ok(if check_only {
            base
        } else {
            append_schema_sync(base, &target_tag)
        });
    }
    if check_only {
        return Ok(format!("Update available: {current_tag} -> {target_tag}"));
    }
    perform_binary_update(&exe_path, &target_tag, release_base_url)
}

fn perform_binary_update(
    exe_path: &Path,
    target_tag: &str,
    release_base_url: Option<&str>,
) -> Result<String, UpdateError> {
    let (target, ext) = detect_target()?;
    let archive_name = format!("codeint-{target}.{ext}");
    let download_base = resolve_download_base(release_base_url, target_tag);
    let stage_dir = create_stage_dir(exe_path)?;
    let result = install_verified_archive(&stage_dir, exe_path, &download_base, &archive_name, ext);
    let _ = fs::remove_dir_all(&stage_dir);
    result?;
    let base = format!("Updated codeint to {target_tag} ({})", exe_path.display());
    Ok(append_schema_sync(base, target_tag))
}

fn append_schema_sync(mut message: String, target_tag: &str) -> String {
    if let Some(updated) = sync_config_schema(target_tag) {
        message.push_str(&format!("\n{updated}"));
    }
    message
}

fn find_config_file(start: &Path) -> Option<codeint_config::DiscoveredConfigFile> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        match codeint_config::discover_config_file(current) {
            Ok(Some(discovered)) => return discovered.path.is_file().then_some(discovered),
            Ok(None) => {}
            Err(_) => return None,
        }
        if current.join(".git").exists() {
            return None;
        }
        dir = current.parent();
    }
    None
}

fn sync_config_schema(target_tag: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let discovered = find_config_file(&cwd)?;
    let text = fs::read_to_string(&discovered.path).ok()?;
    let (updated, new_url) =
        codeint_config::rewrite_schema_tag(discovered.kind, &text, target_tag)?;
    fs::write(&discovered.path, updated).ok()?;
    Some(format_schema_sync_notice(
        discovered.kind,
        &discovered.path,
        &new_url,
    ))
}

fn format_schema_sync_notice(kind: ConfigFileKind, path: &Path, new_url: &str) -> String {
    match kind.syntax() {
        ConfigSyntax::Json => format!("Updated $schema in {} to {new_url}", path.display()),
        ConfigSyntax::Toml => {
            format!(
                "Updated #:schema directive in {} to {new_url}",
                path.display()
            )
        }
        ConfigSyntax::Yaml => format!(
            "Updated # yaml-language-server: $schema= directive in {} to {new_url}",
            path.display()
        ),
    }
}

fn is_node_modules_install(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::Normal(name) if name == "node_modules"))
}

fn normalize_tag(tag: &str) -> String {
    let trimmed = tag.trim();
    if trimmed.starts_with('v') {
        trimmed.to_owned()
    } else {
        format!("v{trimmed}")
    }
}

fn resolve_download_base(release_base_url: Option<&str>, tag: &str) -> String {
    match release_base_url {
        Some(base) => base.trim_end_matches('/').to_owned(),
        None => format!("{DEFAULT_RELEASE_DOWNLOAD_BASE}/{tag}"),
    }
}

fn resolve_latest_tag(release_base_url: Option<&str>) -> Result<String, UpdateError> {
    if let Some(base) = release_base_url {
        let base = base.trim_end_matches('/');
        if let Some(local_dir) = local_path_from_base(base) {
            let latest_path = local_dir.join("LATEST");
            let raw =
                fs::read_to_string(&latest_path).map_err(|error| UpdateError::LatestRelease {
                    detail: format!("cannot read {}: {error}", latest_path.display()),
                })?;
            let tag = raw.trim();
            if tag.is_empty() {
                return Err(UpdateError::LatestRelease {
                    detail: format!("{} is empty", latest_path.display()),
                });
            }
            return Ok(normalize_tag(tag));
        }
        let url = format!("{base}/LATEST");
        let text = fetch_text_http(&url)?;
        let tag = text.trim();
        if tag.is_empty() {
            return Err(UpdateError::LatestRelease {
                detail: format!("{url} returned an empty tag"),
            });
        }
        return Ok(normalize_tag(tag));
    }

    let body = fetch_text_http(LATEST_RELEASE_API_URL)?;
    extract_tag_name(&body)
}

fn extract_tag_name(json_text: &str) -> Result<String, UpdateError> {
    let release: LatestReleaseResponse =
        serde_json::from_str(json_text).map_err(|error| UpdateError::LatestRelease {
            detail: format!("invalid GitHub Releases API JSON: {error}"),
        })?;
    let trimmed = release.tag_name.trim();
    if trimmed.is_empty() {
        return Err(UpdateError::LatestRelease {
            detail: "GitHub Releases API response contained an empty tag_name".to_owned(),
        });
    }
    Ok(normalize_tag(trimmed))
}

fn detect_target() -> Result<(&'static str, &'static str), UpdateError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok(("aarch64-apple-darwin", "tar.gz")),
        ("macos", "x86_64") => Ok(("x86_64-apple-darwin", "tar.gz")),
        ("linux", "x86_64") => Ok(("x86_64-unknown-linux-musl", "tar.gz")),
        ("linux", "aarch64") => Ok(("aarch64-unknown-linux-musl", "tar.gz")),
        ("windows", "x86_64") => Ok(("x86_64-pc-windows-msvc", "zip")),
        ("windows", "aarch64") => Ok(("aarch64-pc-windows-msvc", "zip")),
        _ => Err(UpdateError::UnsupportedPlatform),
    }
}

fn create_stage_dir(exe_path: &Path) -> Result<PathBuf, UpdateError> {
    let parent = exe_path.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let stage = parent.join(format!(".codeint-update-{pid}"));
    if stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    fs::create_dir_all(&stage).map_err(|source| UpdateError::Replace {
        path: stage.clone(),
        source,
    })?;
    Ok(stage)
}

fn install_verified_archive(
    stage_dir: &Path,
    exe_path: &Path,
    download_base: &str,
    archive_name: &str,
    ext: &str,
) -> Result<(), UpdateError> {
    let checksums_path = stage_dir.join("SHA256SUMS");
    let archive_path = stage_dir.join(archive_name);

    fetch_asset(download_base, "SHA256SUMS", &checksums_path)?;
    fetch_asset(download_base, archive_name, &archive_path)?;

    let checksums_text =
        fs::read_to_string(&checksums_path).map_err(|source| UpdateError::Download {
            url: format!("{download_base}/SHA256SUMS"),
            detail: source.to_string(),
        })?;
    let expected = parse_expected_sha256(&checksums_text, archive_name).ok_or_else(|| {
        UpdateError::MissingChecksum {
            file: archive_name.to_owned(),
        }
    })?;
    verify_archive_sha256(&archive_path, archive_name, &expected)?;

    let extract_dir = stage_dir.join("extracted");
    fs::create_dir_all(&extract_dir).map_err(|source| UpdateError::Replace {
        path: extract_dir.clone(),
        source,
    })?;
    extract_archive(&archive_path, &extract_dir, ext)?;

    let binary_name = if cfg!(windows) {
        "codeint.exe"
    } else {
        "codeint"
    };
    let extracted_bin = extract_dir.join(binary_name);
    if !extracted_bin.is_file() {
        return Err(UpdateError::Extract {
            detail: format!("archive did not contain {binary_name}"),
        });
    }

    replace_executable(&extracted_bin, exe_path)
}

fn local_path_from_base(base: &str) -> Option<PathBuf> {
    if let Some(path) = base.strip_prefix("file://") {
        Some(PathBuf::from(path))
    } else if base.starts_with("http://") || base.starts_with("https://") {
        None
    } else {
        Some(PathBuf::from(base))
    }
}

fn build_curl_base_args(max_time_seconds: &str) -> Vec<OsString> {
    [
        "--proto",
        "=https",
        "--tlsv1.2",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        CURL_CONNECT_TIMEOUT_SECONDS,
        "--max-time",
        max_time_seconds,
        "--fail",
        "--silent",
        "--show-error",
        "--location",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn build_curl_download_args(url: &str, destination: &Path) -> Vec<OsString> {
    let mut args = build_curl_base_args(CURL_DOWNLOAD_MAX_TIME_SECONDS);
    args.push(OsString::from("--output"));
    args.push(destination.as_os_str().to_owned());
    args.push(OsString::from("--"));
    args.push(OsString::from(url));
    args
}

fn build_curl_text_args(url: &str) -> Vec<OsString> {
    let mut args = build_curl_base_args(CURL_METADATA_MAX_TIME_SECONDS);
    args.push(OsString::from("--header"));
    args.push(OsString::from("Accept: application/vnd.github+json"));
    args.push(OsString::from("--"));
    args.push(OsString::from(url));
    args
}

fn fetch_asset(base: &str, filename: &str, destination: &Path) -> Result<(), UpdateError> {
    if let Some(dir) = local_path_from_base(base) {
        let source_path = dir.join(filename);
        fs::copy(&source_path, destination).map_err(|error| UpdateError::Download {
            url: source_path.display().to_string(),
            detail: error.to_string(),
        })?;
        return Ok(());
    }
    let url = format!("{base}/{filename}");
    let output = Command::new("curl")
        .args(build_curl_download_args(&url, destination))
        .output()
        .map_err(|error| UpdateError::Download {
            url: url.clone(),
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            format!("curl exited with {}", output.status)
        } else {
            stderr
        };
        return Err(UpdateError::Download { url, detail });
    }
    Ok(())
}

fn fetch_text_http(url: &str) -> Result<String, UpdateError> {
    let output = Command::new("curl")
        .args(build_curl_text_args(url))
        .output()
        .map_err(|error| UpdateError::LatestRelease {
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            format!("curl exited with {}", output.status)
        } else {
            stderr
        };
        return Err(UpdateError::LatestRelease { detail });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_expected_sha256(checksums: &str, archive_name: &str) -> Option<String> {
    for line in checksums.lines() {
        let trimmed = line.trim();
        let mut parts = trimmed.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(file) = parts.next() else {
            continue;
        };
        let normalized_file = file.strip_prefix('*').unwrap_or(file);
        if normalized_file == archive_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

fn compute_sha256(path: &Path) -> Result<[u8; 32], UpdateError> {
    let bytes = fs::read(path).map_err(|error| UpdateError::ChecksumTool {
        detail: error.to_string(),
    })?;
    Ok(sha256(&bytes))
}

fn parse_sha256_hex(hex: &str) -> Option<[u8; 32]> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    let mut digest = [0u8; 32];
    for (slot, pair) in digest.iter_mut().zip(pairs) {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        *slot = (high << 4) | low;
    }
    Some(digest)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn verify_archive_sha256(
    archive_path: &Path,
    archive_name: &str,
    expected_hex: &str,
) -> Result<(), UpdateError> {
    let actual_digest = compute_sha256(archive_path)?;
    let matches = parse_sha256_hex(expected_hex)
        .is_some_and(|expected_digest| constant_time_equal(&actual_digest, &expected_digest));
    if !matches {
        return Err(UpdateError::ChecksumMismatch {
            file: archive_name.to_owned(),
            expected: expected_hex.to_owned(),
            actual: sha256_hex(&actual_digest),
        });
    }
    Ok(())
}

fn extract_archive(archive: &Path, destination: &Path, ext: &str) -> Result<(), UpdateError> {
    let status = if ext == "zip" {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
            ])
            .arg(archive)
            .arg(destination)
            .status()
    } else {
        Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(destination)
            .status()
    }
    .map_err(|error| UpdateError::Extract {
        detail: error.to_string(),
    })?;
    if !status.success() {
        return Err(UpdateError::Extract {
            detail: format!("archive extractor exited with {status}"),
        });
    }
    Ok(())
}

fn replace_executable(staged: &Path, exe_path: &Path) -> Result<(), UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(staged, fs::Permissions::from_mode(0o755)).map_err(|source| {
            UpdateError::Replace {
                path: staged.to_path_buf(),
                source,
            }
        })?;
        fs::rename(staged, exe_path).map_err(|source| UpdateError::Replace {
            path: exe_path.to_path_buf(),
            source,
        })
    }
    #[cfg(windows)]
    {
        let backup = exe_path.with_extension("exe.old");
        let _ = fs::remove_file(&backup);
        fs::rename(exe_path, &backup).map_err(|source| UpdateError::Replace {
            path: exe_path.to_path_buf(),
            source,
        })?;
        if let Err(source) = fs::rename(staged, exe_path) {
            let _ = fs::rename(&backup, exe_path);
            return Err(UpdateError::Replace {
                path: exe_path.to_path_buf(),
                source,
            });
        }
        let _ = fs::remove_file(&backup);
        Ok(())
    }
}
