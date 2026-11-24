use std::collections::BTreeMap;
use std::fmt;

mod apply;
mod cobertura;
mod lcov;
mod matcher;
#[cfg(test)]
mod tests;

pub use apply::{
    CoverageApplyError, CoverageApplyOutcome, NamedCoverageReport, apply_coverage_reports,
};
pub use cobertura::{parse_cobertura, parse_cobertura_with_limits};
pub use lcov::{parse_lcov, parse_lcov_with_limits};
pub use matcher::{PathMatcher, match_coverage_path};

pub const MAX_COVERAGE_REPORT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_XML_NESTING_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageLimits {
    pub max_bytes: usize,
    pub max_xml_depth: usize,
}

impl Default for CoverageLimits {
    fn default() -> CoverageLimits {
        CoverageLimits {
            max_bytes: MAX_COVERAGE_REPORT_BYTES,
            max_xml_depth: MAX_XML_NESTING_DEPTH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageFormat {
    Lcov,
    Cobertura,
}

impl fmt::Display for CoverageFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverageFormat::Lcov => formatter.write_str("LCOV"),
            CoverageFormat::Cobertura => formatter.write_str("Cobertura XML"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageFileRecord {
    pub path: String,
    pub lines: BTreeMap<u32, u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCoverageReport {
    pub format: CoverageFormat,
    pub source_roots: Vec<String>,
    pub files: Vec<CoverageFileRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CoverageParseError {
    #[error("Coverage report is {size_bytes} bytes, exceeding the {max_bytes}-byte limit")]
    ReportTooLarge { size_bytes: usize, max_bytes: usize },
    #[error("Coverage report is not valid UTF-8")]
    InvalidUtf8,
    #[error(
        "Coverage report format is not recognized; supported formats are LCOV and Cobertura XML"
    )]
    UnrecognizedFormat,
    #[error("invalid LCOV record on line {line}: {content:?}")]
    InvalidLcovRecord { line: usize, content: String },
    #[error("invalid LCOV record on line {line}: {reason}")]
    InvalidLcovData { line: usize, reason: String },
    #[error("rejected unsafe Cobertura XML: {reason}")]
    XmlSecurityViolation { reason: String },
    #[error("Cobertura XML exceeds maximum nesting depth of {max_depth}")]
    XmlNestingTooDeep { max_depth: usize },
    #[error("invalid Cobertura XML: {reason}")]
    InvalidCoberturaXml { reason: String },
}

pub fn parse_coverage_report(bytes: &[u8]) -> Result<ParsedCoverageReport, CoverageParseError> {
    parse_coverage_report_with_limits(bytes, CoverageLimits::default())
}

pub fn parse_coverage_report_with_limits(
    bytes: &[u8],
    limits: CoverageLimits,
) -> Result<ParsedCoverageReport, CoverageParseError> {
    let text = decode_report_utf8(bytes, limits.max_bytes)?;
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return Err(CoverageParseError::UnrecognizedFormat);
    }
    if trimmed.starts_with('<') {
        return cobertura::parse_cobertura_str(text, limits.max_xml_depth);
    }
    if lcov::is_lcov_start(trimmed) {
        return lcov::parse_lcov_str(text);
    }
    Err(CoverageParseError::UnrecognizedFormat)
}

pub(crate) fn decode_report_utf8(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<&str, CoverageParseError> {
    if bytes.len() > max_bytes {
        return Err(CoverageParseError::ReportTooLarge {
            size_bytes: bytes.len(),
            max_bytes,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| CoverageParseError::InvalidUtf8)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

pub(crate) fn merge_line_hit(lines: &mut BTreeMap<u32, u64>, line: u32, hits: u64) {
    lines
        .entry(line)
        .and_modify(|existing| *existing = existing.saturating_add(hits))
        .or_insert(hits);
}

pub(crate) fn collect_file_records(
    files: BTreeMap<String, BTreeMap<u32, u64>>,
) -> Vec<CoverageFileRecord> {
    files
        .into_iter()
        .map(|(path, lines)| CoverageFileRecord { path, lines })
        .collect()
}
