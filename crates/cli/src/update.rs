use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/avidianity/codeint/releases/latest";
const DEFAULT_RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/avidianity/codeint/releases/download";
const SCHEMA_URL_PREFIX: &str = "https://raw.githubusercontent.com/avidianity/codeint/";
const SCHEMA_URL_SUFFIX: &str = "/schema/codeint.schema.json";

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
    if let Some(updated) = sync_codeint_json_schema(target_tag) {
        message.push_str(&format!("\n{updated}"));
    }
    message
}

fn find_codeint_json(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        match codeint_config::discover_config_file(current) {
            Ok(Some(discovered)) => return discovered.path.is_file().then_some(discovered.path),
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

fn replace_schema_tag(text: &str, target_tag: &str) -> Option<(String, String)> {
    let schema = codeint_config::schema_value_span(text)?;
    let rest = schema.value.strip_prefix(SCHEMA_URL_PREFIX)?;
    let current_ref = rest.strip_suffix(SCHEMA_URL_SUFFIX)?;
    if current_ref.is_empty() || current_ref.contains('/') || current_ref == target_tag {
        return None;
    }
    let new_url = format!("{SCHEMA_URL_PREFIX}{target_tag}{SCHEMA_URL_SUFFIX}");
    let prefix = text.get(..schema.span.start)?;
    let suffix = text.get(schema.span.end..)?;
    let mut updated = String::with_capacity(text.len() + target_tag.len());
    updated.push_str(prefix);
    updated.push('"');
    updated.push_str(&new_url);
    updated.push('"');
    updated.push_str(suffix);
    Some((updated, new_url))
}

fn sync_codeint_json_schema(target_tag: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let config_path = find_codeint_json(&cwd)?;
    let text = fs::read_to_string(&config_path).ok()?;
    let (updated, new_url) = replace_schema_tag(&text, target_tag)?;
    fs::write(&config_path, updated).ok()?;
    Some(format!(
        "Updated $schema in {} to {new_url}",
        config_path.display()
    ))
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
    extract_tag_name(&body).ok_or_else(|| UpdateError::LatestRelease {
        detail: "GitHub Releases API response did not include tag_name".to_owned(),
    })
}

fn extract_tag_name(json_text: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let idx = json_text.find(key)?;
    let rest = &json_text[idx + key.len()..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    let after_quote = after_colon.strip_prefix('"')?;
    let end_quote = after_quote.find('"')?;
    let tag = &after_quote[..end_quote];
    (!tag.is_empty()).then(|| normalize_tag(tag))
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
    let actual = compute_sha256(&archive_path)?;
    if actual != expected {
        return Err(UpdateError::ChecksumMismatch {
            file: archive_name.to_owned(),
            expected,
            actual,
        });
    }

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
    let status = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(destination)
        .arg(&url)
        .status()
        .map_err(|error| UpdateError::Download {
            url: url.clone(),
            detail: error.to_string(),
        })?;
    if !status.success() {
        return Err(UpdateError::Download {
            url,
            detail: format!("curl exited with {status}"),
        });
    }
    Ok(())
}

fn fetch_text_http(url: &str) -> Result<String, UpdateError> {
    let output = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", url])
        .output()
        .map_err(|error| UpdateError::LatestRelease {
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(UpdateError::LatestRelease {
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
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

fn compute_sha256(path: &Path) -> Result<String, UpdateError> {
    #[cfg(windows)]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-FileHash -Algorithm SHA256 -LiteralPath $args[0]).Hash.ToLowerInvariant()",
            ])
            .arg(path)
            .output()
            .map_err(|error| UpdateError::ChecksumTool {
                detail: error.to_string(),
            })?;
        if !output.status.success() {
            return Err(UpdateError::ChecksumTool {
                detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    #[cfg(not(windows))]
    {
        let attempt = Command::new("sha256sum").arg(path).output().or_else(|_| {
            Command::new("shasum")
                .args(["-a", "256"])
                .arg(path)
                .output()
        });
        let output = attempt.map_err(|error| UpdateError::ChecksumTool {
            detail: error.to_string(),
        })?;
        if !output.status.success() {
            return Err(UpdateError::ChecksumTool {
                detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .split_whitespace()
            .next()
            .map(|hash| hash.to_ascii_lowercase())
            .ok_or_else(|| UpdateError::ChecksumTool {
                detail: "checksum tool returned empty output".to_owned(),
            })
    }
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
