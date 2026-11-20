use codeint_metrics::duplication::Duplication;
use codeint_model::CodeModel;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    duplication: DuplicationReport,
}

#[derive(Debug, Serialize)]
struct DuplicationReport {
    ratio: f64,
    clone_lines: u64,
    clone_groups: Vec<CloneGroupReport>,
}

#[derive(Debug, Serialize)]
struct CloneGroupReport {
    tokens: usize,
    locations: Vec<LocationReport>,
}

#[derive(Debug, Serialize)]
struct LocationReport {
    path: String,
    start_line: u32,
    end_line: u32,
}

impl DuplicationReport {
    fn summarize(duplication: &Duplication) -> DuplicationReport {
        let _ = duplication;
        todo!()
    }
}

impl Report {
    pub fn summarize(model: &CodeModel, duplication: &Duplication) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            duplication: DuplicationReport::summarize(duplication),
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}
