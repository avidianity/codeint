use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use codeint_metrics::{
    CoverageApplyOutcome, CoverageParseError, MAX_COVERAGE_REPORT_BYTES, NamedCoverageReport,
    apply_coverage_reports, parse_coverage_report,
};
use codeint_model::RepoPath;
use codeint_plugin_host::{sha256, sha256_hex};
use codeint_report::MeasuredLanguage;
use serde::Serialize;

use super::CheckError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedCoverageReportPath {
    pub(crate) display: String,
    pub(crate) full: PathBuf,
}

#[derive(Serialize)]
pub(crate) struct CoverageReportDigest {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

pub(crate) fn select_coverage_report_paths(
    folder: &Path,
    repository_root: &Path,
    configured: &[String],
    cli_override: &[PathBuf],
) -> Vec<ResolvedCoverageReportPath> {
    if !cli_override.is_empty() {
        return cli_override
            .iter()
            .map(|raw| ResolvedCoverageReportPath {
                display: raw.display().to_string(),
                full: resolve_cli_coverage_path(folder, repository_root, raw),
            })
            .collect();
    }
    configured
        .iter()
        .map(|entry| ResolvedCoverageReportPath {
            display: entry.clone(),
            full: repository_root.join(entry),
        })
        .collect()
}

fn resolve_cli_coverage_path(folder: &Path, repository_root: &Path, raw: &Path) -> PathBuf {
    if raw.is_absolute() || raw.exists() {
        return raw.to_path_buf();
    }
    let under_folder = folder.join(raw);
    if under_folder.exists() {
        return under_folder;
    }
    repository_root.join(raw)
}

pub(crate) fn load_coverage_reports(
    paths: &[ResolvedCoverageReportPath],
) -> Result<Vec<NamedCoverageReport>, CheckError> {
    paths
        .iter()
        .map(|target| {
            let bytes = read_bounded_coverage_file(&target.display, &target.full)?;
            let report = parse_coverage_report(&bytes).map_err(|source| {
                CheckError::InvalidCoverageReport {
                    path: target.display.clone(),
                    source,
                }
            })?;
            Ok(NamedCoverageReport {
                path: target.display.clone(),
                report,
            })
        })
        .collect()
}

pub(crate) fn apply_loaded_coverage_reports(
    reports: &[NamedCoverageReport],
    languages: &mut [MeasuredLanguage],
    file_line_counts: &BTreeMap<RepoPath, usize>,
) -> Result<CoverageApplyOutcome, CheckError> {
    if reports.is_empty() {
        return Ok(CoverageApplyOutcome::default());
    }
    let mut models: Vec<_> = languages
        .iter_mut()
        .map(|language| std::mem::take(&mut language.model))
        .collect();
    let outcome = apply_coverage_reports(reports, &mut models, file_line_counts);
    for (language, model) in languages.iter_mut().zip(models) {
        language.model = model;
    }
    Ok(outcome?)
}

pub(crate) fn hash_configured_coverage_reports(
    repository_root: &Path,
    configured: &[String],
) -> Vec<CoverageReportDigest> {
    configured
        .iter()
        .map(|entry| {
            let full = repository_root.join(entry);
            let digest = read_bounded_coverage_file(entry, &full)
                .ok()
                .map(|bytes| sha256_hex(&sha256(&bytes)));
            CoverageReportDigest {
                path: entry.clone(),
                sha256: digest,
            }
        })
        .collect()
}

fn read_bounded_coverage_file(display_path: &str, full_path: &Path) -> Result<Vec<u8>, CheckError> {
    let file = std::fs::File::open(full_path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            CheckError::MissingCoverageReport {
                path: display_path.to_owned(),
            }
        } else {
            CheckError::UnreadableCoverageReport {
                path: display_path.to_owned(),
                source,
            }
        }
    })?;
    if let Ok(metadata) = file.metadata() {
        let len = metadata.len();
        let max_bytes = MAX_COVERAGE_REPORT_BYTES as u64;
        if len > max_bytes {
            let size_bytes = usize::try_from(len).unwrap_or(usize::MAX);
            return Err(CheckError::InvalidCoverageReport {
                path: display_path.to_owned(),
                source: CoverageParseError::ReportTooLarge {
                    size_bytes,
                    max_bytes: MAX_COVERAGE_REPORT_BYTES,
                },
            });
        }
    }
    let limit = (MAX_COVERAGE_REPORT_BYTES as u64).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).map_err(|source| {
        CheckError::UnreadableCoverageReport {
            path: display_path.to_owned(),
            source,
        }
    })?;
    if bytes.len() > MAX_COVERAGE_REPORT_BYTES {
        return Err(CheckError::InvalidCoverageReport {
            path: display_path.to_owned(),
            source: CoverageParseError::ReportTooLarge {
                size_bytes: bytes.len(),
                max_bytes: MAX_COVERAGE_REPORT_BYTES,
            },
        });
    }
    Ok(bytes)
}
