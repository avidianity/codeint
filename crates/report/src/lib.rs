#![forbid(unsafe_code)]

use codeint_metrics::{ErodedFunction, Erosion};
use codeint_model::{CodeModel, RepoPath, SkippedFile, SourceSet, SourceSets};
use serde::Serialize;

const DECIMAL_SCALE: f64 = 10_000.0;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    source_sets: SourceSetSections,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct SourceSetSections {
    #[serde(skip_serializing_if = "Option::is_none")]
    production: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendored: Option<SourceSetSection>,
}

#[derive(Debug, Serialize)]
struct SourceSetSection {
    files: usize,
    source_lines: u64,
    erosion: ErosionSection,
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

impl SourceSetSections {
    fn new(source_sets: &SourceSets) -> SourceSetSections {
        let mut sections = SourceSetSections::default();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                *sections.slot(source_set) = Some(SourceSetSection::new(model));
            }
        }
        sections
    }

    fn slot(&mut self, source_set: SourceSet) -> &mut Option<SourceSetSection> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

impl SourceSetSection {
    fn new(model: &CodeModel) -> SourceSetSection {
        SourceSetSection {
            files: model.files.len(),
            source_lines: source_lines(model),
            erosion: ErosionSection::new(&Erosion::measure(model)),
        }
    }
}

fn source_lines(model: &CodeModel) -> u64 {
    model
        .files
        .iter()
        .map(|file| u64::from(file.model.source_lines))
        .sum()
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
    pub fn summarize(model: CodeModel) -> Report {
        Report {
            files: model.files.len(),
            source_lines: source_lines(&model),
            skipped: model.skipped.iter().map(SkippedEntry::from).collect(),
            declaration_only: model
                .declaration_only
                .iter()
                .map(RepoPath::to_string)
                .collect(),
            source_sets: SourceSetSections::new(&model.into_source_sets()),
        }
    }

    pub fn to_toon(&self) -> Result<String, ReportError> {
        toon_format::encode_default(self).map_err(ReportError)
    }
}
