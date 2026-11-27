use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use codeint_config::{PluginDeclaration, PluginHostConfig};

use crate::sha256::{format_hex, sha256};
use crate::{PluginHostError, constant_time_equal};

#[cfg(test)]
mod tests;

const CURL_FILESIZE_EXCEEDED_EXIT_CODE: i32 = 63;
const DOWNLOAD_CHUNK_BYTES: usize = 64 * 1024;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn load_https_plugin(
    declaration: &PluginDeclaration,
    url: &str,
    host_config: &PluginHostConfig,
) -> Result<(PathBuf, Vec<u8>), PluginHostError> {
    if !is_https_url(url) {
        return Err(PluginHostError::NonHttpsUrl {
            name: declaration.name.clone(),
            url: url.to_owned(),
        });
    }
    let cache_dir =
        host_config
            .cache_dir
            .as_deref()
            .ok_or_else(|| PluginHostError::MissingCacheDir {
                name: declaration.name.clone(),
                url: url.to_owned(),
            })?;
    let expected_hex = declaration.sha256.to_hex();
    let cached_path = cache_dir.join(format!("{expected_hex}.wasm"));
    if let Some(bytes) = read_valid_cache_entry(&cached_path, declaration.sha256.as_bytes()) {
        return Ok((cached_path, bytes));
    }

    std::fs::create_dir_all(cache_dir).map_err(|source| PluginHostError::CacheWrite {
        name: declaration.name.clone(),
        path: cache_dir.to_path_buf(),
        source,
    })?;
    let temp_path = unique_temp_path(cache_dir, &expected_hex);
    download_to_file(declaration, url, host_config, &temp_path)?;
    let bytes = std::fs::read(&temp_path).map_err(|source| {
        let _ = std::fs::remove_file(&temp_path);
        PluginHostError::Read {
            name: declaration.name.clone(),
            path: temp_path.clone(),
            source,
        }
    })?;
    let actual = sha256(&bytes);
    if !constant_time_equal(&actual, declaration.sha256.as_bytes()) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(PluginHostError::DownloadHashMismatch {
            name: declaration.name.clone(),
            url: url.to_owned(),
            expected: expected_hex,
            actual: format_hex(&actual),
        });
    }
    promote_to_cache(
        &declaration.name,
        &temp_path,
        &cached_path,
        declaration.sha256.as_bytes(),
    )?;
    Ok((cached_path, bytes))
}

fn is_https_url(url: &str) -> bool {
    let Some(prefix) = url.get(..8) else {
        return false;
    };
    if !prefix.eq_ignore_ascii_case("https://") {
        return false;
    }
    let rest = &url[8..];
    !rest.is_empty() && !rest.starts_with('/') && !url.chars().any(char::is_whitespace)
}

fn read_valid_cache_entry(cached_path: &Path, expected: &[u8; 32]) -> Option<Vec<u8>> {
    let bytes = std::fs::read(cached_path).ok()?;
    let actual = sha256(&bytes);
    constant_time_equal(&actual, expected).then_some(bytes)
}

fn unique_temp_path(cache_dir: &Path, expected_hex: &str) -> PathBuf {
    let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    cache_dir.join(format!(
        ".{expected_hex}.{}.{sequence}.tmp",
        std::process::id()
    ))
}

fn download_to_file(
    declaration: &PluginDeclaration,
    url: &str,
    host_config: &PluginHostConfig,
    temp_path: &Path,
) -> Result<(), PluginHostError> {
    let timeout = host_config.download_timeout_seconds.to_string();
    let max_bytes = host_config.max_download_bytes;
    let max_filesize = max_bytes.to_string();
    let mut command = Command::new("curl");
    command
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--proto-redir",
            "=https",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            &timeout,
            "--max-filesize",
            &max_filesize,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(ca_cert) = &host_config.ca_cert {
        command.arg("--cacert").arg(ca_cert);
    }
    command.arg("--").arg(url);

    let mut child = command
        .spawn()
        .map_err(|source| PluginHostError::Download {
            name: declaration.name.clone(),
            url: url.to_owned(),
            reason: format!("cannot invoke curl: {source}"),
        })?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| PluginHostError::Download {
            name: declaration.name.clone(),
            url: url.to_owned(),
            reason: "cannot capture curl stdout".to_owned(),
        })?;
    let mut temp_file = File::create(temp_path).map_err(|source| {
        abort_download(&mut child, temp_path);
        PluginHostError::CacheWrite {
            name: declaration.name.clone(),
            path: temp_path.to_path_buf(),
            source,
        }
    })?;

    let copy_result = copy_bounded(&mut stdout, &mut temp_file, max_bytes);
    drop(temp_file);
    drop(stdout);

    match copy_result {
        Ok(BoundedCopy::Exceeded) => {
            abort_download(&mut child, temp_path);
            return Err(PluginHostError::DownloadTooLarge {
                name: declaration.name.clone(),
                url: url.to_owned(),
                max_bytes,
            });
        }
        Err(BoundedCopyError::Read(source)) => {
            abort_download(&mut child, temp_path);
            return Err(PluginHostError::Download {
                name: declaration.name.clone(),
                url: url.to_owned(),
                reason: format!("cannot read curl stdout: {source}"),
            });
        }
        Err(BoundedCopyError::Write(source)) => {
            abort_download(&mut child, temp_path);
            return Err(PluginHostError::CacheWrite {
                name: declaration.name.clone(),
                path: temp_path.to_path_buf(),
                source,
            });
        }
        Ok(BoundedCopy::Completed) => {}
    }

    let output = child.wait_with_output().map_err(|source| {
        let _ = std::fs::remove_file(temp_path);
        PluginHostError::Download {
            name: declaration.name.clone(),
            url: url.to_owned(),
            reason: format!("cannot wait for curl: {source}"),
        }
    })?;
    if !output.status.success() {
        let _ = std::fs::remove_file(temp_path);
        if output.status.code() == Some(CURL_FILESIZE_EXCEEDED_EXIT_CODE) {
            return Err(PluginHostError::DownloadTooLarge {
                name: declaration.name.clone(),
                url: url.to_owned(),
                max_bytes,
            });
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let reason = if stderr.is_empty() {
            format!("curl exited with {}", output.status)
        } else {
            stderr
        };
        return Err(PluginHostError::Download {
            name: declaration.name.clone(),
            url: url.to_owned(),
            reason,
        });
    }
    Ok(())
}

fn abort_download(child: &mut std::process::Child, temp_path: &Path) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(temp_path);
}

#[derive(Debug)]
enum BoundedCopy {
    Completed,
    Exceeded,
}

#[derive(Debug)]
enum BoundedCopyError {
    Read(std::io::Error),
    Write(std::io::Error),
}

fn copy_bounded(
    reader: &mut impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
) -> Result<BoundedCopy, BoundedCopyError> {
    let mut buffer = [0u8; DOWNLOAD_CHUNK_BYTES];
    let mut total: u64 = 0;
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(BoundedCopyError::Read(error)),
        };
        if read == 0 {
            return Ok(BoundedCopy::Completed);
        }
        total = total.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if total > max_bytes {
            return Ok(BoundedCopy::Exceeded);
        }
        writer
            .write_all(&buffer[..read])
            .map_err(BoundedCopyError::Write)?;
    }
}

fn promote_to_cache(
    name: &str,
    temp_path: &Path,
    cached_path: &Path,
    expected: &[u8; 32],
) -> Result<(), PluginHostError> {
    match std::fs::rename(temp_path, cached_path) {
        Ok(()) => Ok(()),
        Err(source) => {
            let _ = std::fs::remove_file(temp_path);
            if read_valid_cache_entry(cached_path, expected).is_some() {
                Ok(())
            } else {
                Err(PluginHostError::CacheWrite {
                    name: name.to_owned(),
                    path: cached_path.to_path_buf(),
                    source,
                })
            }
        }
    }
}
