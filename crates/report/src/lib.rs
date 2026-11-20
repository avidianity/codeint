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
    pub fn summarize(_model: &CodeModel) -> Report {
        todo!()
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        todo!()
    }
}
