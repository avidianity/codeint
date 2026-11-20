#![forbid(unsafe_code)]

use codeint_model::CodeModel;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
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
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}
