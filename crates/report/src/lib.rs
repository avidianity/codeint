use codeint_metrics::verbosity::{FlaggedLineFinding, Verbosity};
use codeint_model::CodeModel;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    verbosity: VerbosityReport,
}

#[derive(Debug, Serialize)]
struct VerbosityReport {
    ratio: f64,
    verbose_lines: u64,
    flagged_lines: Vec<FlaggedLineRow>,
}

#[derive(Debug, Serialize)]
struct FlaggedLineRow {
    path: String,
    line: u32,
    rule: String,
}

impl Report {
    pub fn summarize(model: &CodeModel) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            verbosity: VerbosityReport::from(Verbosity::measure(model)),
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}

impl From<Verbosity> for VerbosityReport {
    fn from(verbosity: Verbosity) -> VerbosityReport {
        todo!("{verbosity:?}")
    }
}

impl From<FlaggedLineFinding> for FlaggedLineRow {
    fn from(finding: FlaggedLineFinding) -> FlaggedLineRow {
        todo!("{finding:?}")
    }
}
