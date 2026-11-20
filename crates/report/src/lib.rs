#![forbid(unsafe_code)]

use codeint_metrics::import_cycles::ImportCycles;
use codeint_model::CodeModel;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    import_cycles: ImportCyclesSection,
}

#[derive(Debug, Serialize)]
struct ImportCyclesSection {
    ratio: f64,
    modules_in_import_cycles: usize,
    modules: usize,
    findings: Vec<ImportCycleFinding>,
}

#[derive(Debug, Serialize)]
struct ImportCycleFinding {
    modules: Vec<String>,
}

impl Report {
    pub fn summarize(model: &CodeModel, import_cycles: &ImportCycles) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            import_cycles: ImportCyclesSection::from_metric(import_cycles),
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}

impl ImportCyclesSection {
    fn from_metric(_import_cycles: &ImportCycles) -> ImportCyclesSection {
        todo!()
    }
}
