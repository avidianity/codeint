use std::collections::BTreeMap;

use super::{
    CoverageFormat, CoverageParseError, MAX_COVERAGE_REPORT_BYTES, ParsedCoverageReport,
    collect_file_records, decode_report_utf8, merge_line_hit,
};

const IGNORED_LCOV_TAGS: [&str; 10] = [
    "TN", "FN", "FNDA", "FNF", "FNH", "BRDA", "BRF", "BRH", "LF", "LH",
];

pub fn parse_lcov(bytes: &[u8]) -> Result<ParsedCoverageReport, CoverageParseError> {
    parse_lcov_with_limits(bytes, MAX_COVERAGE_REPORT_BYTES)
}

pub fn parse_lcov_with_limits(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<ParsedCoverageReport, CoverageParseError> {
    let text = decode_report_utf8(bytes, max_bytes)?;
    parse_lcov_str(text)
}

pub(crate) fn is_lcov_start(trimmed: &str) -> bool {
    let first_line = trimmed
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .find(|line| !line.is_empty());
    let Some(line) = first_line else {
        return false;
    };
    if line == "end_of_record" {
        return true;
    }
    let Some((tag, _)) = line.split_once(':') else {
        return false;
    };
    tag == "SF" || tag == "DA" || IGNORED_LCOV_TAGS.contains(&tag)
}

pub(crate) fn parse_lcov_str(text: &str) -> Result<ParsedCoverageReport, CoverageParseError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current_file: Option<String> = None;
    let mut saw_any_record = false;
    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        saw_any_record = true;
        parse_lcov_line(line, line_number, &mut current_file, &mut files)?;
    }
    if !saw_any_record {
        return Err(CoverageParseError::UnrecognizedFormat);
    }
    Ok(ParsedCoverageReport {
        format: CoverageFormat::Lcov,
        source_roots: Vec::new(),
        files: collect_file_records(files),
    })
}

fn parse_lcov_line(
    line: &str,
    line_number: usize,
    current_file: &mut Option<String>,
    files: &mut BTreeMap<String, BTreeMap<u32, u64>>,
) -> Result<(), CoverageParseError> {
    if line == "end_of_record" {
        *current_file = None;
        return Ok(());
    }
    let Some((tag, value)) = line.split_once(':') else {
        return Err(CoverageParseError::InvalidLcovRecord {
            line: line_number,
            content: line.to_owned(),
        });
    };
    match tag {
        "SF" => {
            let path = value.trim();
            if path.is_empty() {
                return Err(CoverageParseError::InvalidLcovData {
                    line: line_number,
                    reason: "SF record has an empty file path".to_owned(),
                });
            }
            files.entry(path.to_owned()).or_default();
            *current_file = Some(path.to_owned());
            Ok(())
        }
        "DA" => {
            let Some(path) = current_file.as_deref() else {
                return Err(CoverageParseError::InvalidLcovData {
                    line: line_number,
                    reason: "DA record appears outside an SF section".to_owned(),
                });
            };
            let (target_line, hits) = parse_da_fields(value, line_number)?;
            let lines = files.entry(path.to_owned()).or_default();
            merge_line_hit(lines, target_line, hits);
            Ok(())
        }
        _ if IGNORED_LCOV_TAGS.contains(&tag) => Ok(()),
        _ => Err(CoverageParseError::InvalidLcovRecord {
            line: line_number,
            content: line.to_owned(),
        }),
    }
}

fn parse_da_fields(value: &str, line_number: usize) -> Result<(u32, u64), CoverageParseError> {
    let mut parts = value.split(',');
    let line_part = parts.next().unwrap_or("");
    let hits_part = parts
        .next()
        .ok_or_else(|| CoverageParseError::InvalidLcovData {
            line: line_number,
            reason: "DA record must contain line number and hit count".to_owned(),
        })?;
    let _checksum = parts.next();
    if parts.next().is_some() {
        return Err(CoverageParseError::InvalidLcovData {
            line: line_number,
            reason: "DA record has too many comma-separated fields".to_owned(),
        });
    }
    let target_line: u32 =
        line_part
            .trim()
            .parse()
            .map_err(|_| CoverageParseError::InvalidLcovData {
                line: line_number,
                reason: format!("invalid DA line number {line_part:?}"),
            })?;
    if target_line == 0 {
        return Err(CoverageParseError::InvalidLcovData {
            line: line_number,
            reason: "DA line number must be positive".to_owned(),
        });
    }
    let hits: u64 = hits_part
        .trim()
        .parse()
        .map_err(|_| CoverageParseError::InvalidLcovData {
            line: line_number,
            reason: format!("invalid DA hit count {hits_part:?}"),
        })?;
    Ok((target_line, hits))
}
