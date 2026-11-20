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
    fn from_metric(import_cycles: &ImportCycles) -> ImportCyclesSection {
        let modules_in_import_cycles = import_cycles.modules_in_import_cycles();
        let modules = import_cycles.modules();
        ImportCyclesSection {
            ratio: ratio(modules_in_import_cycles, modules),
            modules_in_import_cycles,
            modules,
            findings: import_cycles
                .cycles()
                .iter()
                .map(|cycle| ImportCycleFinding {
                    modules: cycle.modules().iter().map(ToString::to_string).collect(),
                })
                .collect(),
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}
