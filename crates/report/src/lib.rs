#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use codeint_metrics::{ErodedFunction, Erosion};
use codeint_model::{CodeModel, Package, RepoPath, SkippedFile};
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    packages: Vec<PackageSection>,
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
struct PackageSection {
    path: String,
    files: usize,
    source_lines: u64,
    erosion: PackageErosion,
}

#[derive(Debug, Serialize)]
struct PackageErosion {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
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

fn package_sections(model: &CodeModel, erosion: &Erosion) -> Vec<PackageSection> {
    let workspace = &model.workspace;
    if !workspace.has_members() {
        return Vec::new();
    }
    let mut sizes: BTreeMap<Package<'_>, (usize, u64)> = BTreeMap::new();
    for file in &model.files {
        let (files, source_lines) = sizes.entry(workspace.package_of(&file.path)).or_default();
        *files += 1;
        *source_lines += u64::from(file.model.source_lines);
    }
    erosion
        .by_package(workspace)
        .into_iter()
        .map(|(package, masses)| {
            let (files, source_lines) = sizes.get(&package).copied().unwrap_or_default();
            PackageSection {
                path: package.to_string(),
                files,
                source_lines,
                erosion: PackageErosion {
                    ratio: Decimal::rounded(masses.ratio()),
                    eroded_function_mass: Decimal::rounded(masses.eroded),
                    total_function_mass: Decimal::rounded(masses.total),
                },
            }
        })
        .collect()
}

impl Report {
    pub fn summarize(model: &CodeModel) -> Report {
        let erosion = Erosion::measure(model);
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            packages: package_sections(model, &erosion),
            erosion: ErosionSection::new(&erosion),
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
