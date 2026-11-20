#![forbid(unsafe_code)]

use codeint_metrics::{ErodedFunction, Erosion, ImportCycles};
use codeint_model::{CodeModel, RepoPath, SkippedFile};
use serde::Serialize;

const DECIMAL_SCALE: f64 = 10_000.0;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    erosion: ErosionSection,
    import_cycles: ImportCyclesSection,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ErosionSection {
    ratio: Decimal,
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
struct ImportCyclesSection {
    ratio: Decimal,
    modules_in_import_cycles: usize,
    modules: usize,
    findings: Vec<ImportCycleFinding>,
}

#[derive(Debug, Serialize)]
struct ImportCycleFinding {
    modules: Vec<String>,
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

impl ErosionSection {
    fn new(erosion: &Erosion) -> ErosionSection {
        ErosionSection {
            ratio: Decimal::rounded(erosion.ratio()),
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

impl ImportCyclesSection {
    fn new(import_cycles: &ImportCycles) -> ImportCyclesSection {
        ImportCyclesSection {
            ratio: Decimal::rounded(import_cycles.ratio()),
            modules_in_import_cycles: import_cycles.modules_in_import_cycles(),
            modules: import_cycles.modules(),
            findings: import_cycles
                .cycles()
                .iter()
                .map(|cycle| ImportCycleFinding {
                    modules: cycle.modules().iter().map(RepoPath::to_string).collect(),
                })
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
    pub fn summarize(model: &CodeModel) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            erosion: ErosionSection::new(&Erosion::measure(model)),
            import_cycles: ImportCyclesSection::new(&ImportCycles::measure(model)),
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
