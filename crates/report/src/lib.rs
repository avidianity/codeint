#![forbid(unsafe_code)]

use codeint_metrics::{ErodedFunction, Erosion};
use codeint_model::{CodeModel, ReferenceRange, ReferenceRanges, RepoPath, SkippedFile};
use serde::Serialize;

const DECIMAL_SCALE: f64 = 10_000.0;
const NOT_PUBLISHED_FOR_THIS_LANGUAGE: &str = "no published Reference range for this language";
const NOT_SHOWN_FOR_SEVERAL_LANGUAGES: &str =
    "not shown, because the measured files are in more than one language";

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    erosion: ErosionSection,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
}

pub enum ReferenceRangeScope {
    OneLanguage(ReferenceRanges),
    SeveralLanguages,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ReferenceRangeEntry {
    Published {
        low: Decimal,
        high: Decimal,
        source: String,
    },
    NotShown(&'static str),
}

#[derive(Debug, Serialize)]
struct ErosionSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
    findings: Vec<ErodedFunctionFinding>,
}

#[derive(Debug, Serialize)]
struct ErodedFunctionFinding {
    path: String,
    line: u32,
    cyclomatic_complexity: u32,
    source_lines: u32,
    function_mass: Decimal,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
struct Decimal(f64);

#[derive(Debug, Serialize)]
struct SkippedEntry {
    path: String,
    reason: String,
}

impl Decimal {
    fn rounded(value: f64) -> Decimal {
        Decimal((value * DECIMAL_SCALE).round() / DECIMAL_SCALE)
    }
}

impl ReferenceRangeScope {
    fn entry(&self, metric: &str) -> ReferenceRangeEntry {
        match self {
            ReferenceRangeScope::OneLanguage(ranges) => ranges.for_metric(metric).map_or(
                ReferenceRangeEntry::NotShown(NOT_PUBLISHED_FOR_THIS_LANGUAGE),
                ReferenceRangeEntry::from,
            ),
            ReferenceRangeScope::SeveralLanguages => {
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_SEVERAL_LANGUAGES)
            }
        }
    }
}

impl From<&ReferenceRange> for ReferenceRangeEntry {
    fn from(range: &ReferenceRange) -> ReferenceRangeEntry {
        ReferenceRangeEntry::Published {
            low: Decimal::rounded(range.low),
            high: Decimal::rounded(range.high),
            source: range.source.clone(),
        }
    }
}

impl ErosionSection {
    fn new(erosion: &Erosion, ranges: &ReferenceRangeScope) -> ErosionSection {
        ErosionSection {
            ratio: Decimal::rounded(erosion.ratio()),
            reference_range: ranges.entry("erosion"),
            eroded_function_mass: Decimal::rounded(erosion.eroded_function_mass()),
            total_function_mass: Decimal::rounded(erosion.total_function_mass()),
            findings: erosion
                .eroded_functions()
                .iter()
                .map(ErodedFunctionFinding::new)
                .collect(),
        }
    }
}

impl ErodedFunctionFinding {
    fn new(function: &ErodedFunction) -> ErodedFunctionFinding {
        ErodedFunctionFinding {
            path: function.path.to_string(),
            line: function.line,
            cyclomatic_complexity: function.cyclomatic_complexity,
            source_lines: function.source_lines,
            function_mass: Decimal::rounded(function.function_mass),
        }
    }
}

impl From<&SkippedFile> for SkippedEntry {
    fn from(file: &SkippedFile) -> SkippedEntry {
        SkippedEntry {
            path: file.path.to_string(),
            reason: file.reason.clone(),
        }
    }
}

impl Report {
    pub fn summarize(model: &CodeModel, ranges: &ReferenceRangeScope) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            erosion: ErosionSection::new(&Erosion::measure(model), ranges),
            skipped: model.skipped.iter().map(SkippedEntry::from).collect(),
            declaration_only: model
                .declaration_only
                .iter()
                .map(RepoPath::to_string)
                .collect(),
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}
