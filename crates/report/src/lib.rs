#![forbid(unsafe_code)]

use codeint_config::FindingsShown;
use codeint_metrics::{
    CloneGroup, Duplication, ErodedFunction, Erosion, FlaggedLineFinding, Fragment, ImportCycle,
    ImportCycles, MinimumCloneTokens, Verbosity,
};
use codeint_model::{CodeModel, RepoPath, SkippedFile};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

const DECIMAL_SCALE: f64 = 10_000.0;

#[derive(Debug, thiserror::Error)]
#[error("cannot render the report")]
pub struct ReportError(#[source] toon_format::ToonError);

#[derive(Debug, Serialize)]
pub struct Report {
    files: usize,
    source_lines: u64,
    erosion: ErosionSection,
    duplication: DuplicationSection,
    verbosity: VerbositySection,
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
    #[serde(flatten)]
    findings: Findings<ErodedFunctionFinding>,
}

trait FindingKind: Serialize {
    const LIST_KEY: &'static str;
}

#[derive(Debug)]
struct Findings<T> {
    listed: Vec<T>,
    left_out: usize,
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
struct DuplicationSection {
    ratio: Decimal,
    clone_lines: u64,
    #[serde(flatten)]
    clone_groups: Findings<CloneGroupFinding>,
}

#[derive(Debug, Serialize)]
struct CloneGroupFinding {
    tokens: usize,
    locations: Vec<CloneLocation>,
}

#[derive(Debug, Serialize)]
struct CloneLocation {
    path: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Debug, Serialize)]
struct VerbositySection {
    ratio: Decimal,
    verbose_lines: u64,
    #[serde(flatten)]
    findings: Findings<FlaggedLineRow>,
}

#[derive(Debug, Serialize)]
struct FlaggedLineRow {
    path: String,
    line: u32,
    rule: String,
}

#[derive(Debug, Serialize)]
struct ImportCyclesSection {
    ratio: Decimal,
    modules_in_import_cycles: usize,
    modules: usize,
    #[serde(flatten)]
    findings: Findings<ImportCycleFinding>,
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

impl<T> Findings<T> {
    fn worst_first(
        worst_first: impl ExactSizeIterator<Item = T>,
        shown: FindingsShown,
    ) -> Findings<T> {
        let total = worst_first.len();
        let listed: Vec<T> = match shown {
            FindingsShown::WorstPerMetric(limit) => worst_first.take(limit).collect(),
            FindingsShown::All => worst_first.collect(),
        };
        Findings {
            left_out: total - listed.len(),
            listed,
        }
    }
}

impl<T: FindingKind> Serialize for Findings<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(T::LIST_KEY, &self.listed)?;
        if self.left_out > 0 {
            map.serialize_entry("findings_left_out", &self.left_out)?;
        }
        map.end()
    }
}

impl FindingKind for ErodedFunctionFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for CloneGroupFinding {
    const LIST_KEY: &'static str = "clone_groups";
}

impl FindingKind for FlaggedLineRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for ImportCycleFinding {
    const LIST_KEY: &'static str = "findings";
}

impl ErosionSection {
    fn new(erosion: &Erosion, shown: FindingsShown) -> ErosionSection {
        ErosionSection {
            ratio: Decimal::rounded(erosion.ratio()),
            eroded_function_mass: Decimal::rounded(erosion.eroded_function_mass()),
            total_function_mass: Decimal::rounded(erosion.total_function_mass()),
            findings: Findings::worst_first(
                erosion
                    .eroded_functions()
                    .iter()
                    .map(ErodedFunctionFinding::new),
                shown,
            ),
        }
    }
}

impl ImportCyclesSection {
    fn new(import_cycles: &ImportCycles, shown: FindingsShown) -> ImportCyclesSection {
        ImportCyclesSection {
            ratio: Decimal::rounded(import_cycles.ratio()),
            modules_in_import_cycles: import_cycles.modules_in_import_cycles(),
            modules: import_cycles.modules(),
            findings: Findings::worst_first(
                import_cycles.cycles().iter().map(ImportCycleFinding::new),
                shown,
            ),
        }
    }
}

impl ImportCycleFinding {
    fn new(cycle: &ImportCycle) -> ImportCycleFinding {
        ImportCycleFinding {
            modules: cycle.modules().iter().map(RepoPath::to_string).collect(),
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

impl DuplicationSection {
    fn new(duplication: &Duplication, shown: FindingsShown) -> DuplicationSection {
        DuplicationSection {
            ratio: Decimal::rounded(duplication.ratio()),
            clone_lines: duplication.clone_lines,
            clone_groups: Findings::worst_first(
                duplication.clone_groups.iter().map(CloneGroupFinding::new),
                shown,
            ),
        }
    }
}

impl CloneGroupFinding {
    fn new(group: &CloneGroup) -> CloneGroupFinding {
        CloneGroupFinding {
            tokens: group.tokens,
            locations: group.fragments.iter().map(CloneLocation::new).collect(),
        }
    }
}

impl CloneLocation {
    fn new(fragment: &Fragment) -> CloneLocation {
        CloneLocation {
            path: fragment.path.to_string(),
            start_line: fragment.start_line,
            end_line: fragment.end_line,
        }
    }
}

impl VerbositySection {
    fn new(verbosity: &Verbosity, shown: FindingsShown) -> VerbositySection {
        VerbositySection {
            ratio: Decimal::rounded(verbosity.ratio()),
            verbose_lines: verbosity.verbose_lines(),
            findings: Findings::worst_first(
                verbosity.flagged_lines().iter().map(FlaggedLineRow::from),
                shown,
            ),
        }
    }
}

impl From<&FlaggedLineFinding> for FlaggedLineRow {
    fn from(finding: &FlaggedLineFinding) -> FlaggedLineRow {
        FlaggedLineRow {
            path: finding.path.to_string(),
            line: finding.line,
            rule: finding.rule.clone(),
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
    pub fn summarize(model: &CodeModel, findings: FindingsShown) -> Report {
        Report {
            files: model.files.len(),
            source_lines: model
                .files
                .iter()
                .map(|file| u64::from(file.model.source_lines))
                .sum(),
            erosion: ErosionSection::new(&Erosion::measure(model), findings),
            duplication: DuplicationSection::new(
                &Duplication::measure(model, MinimumCloneTokens::PROVISIONAL_DEFAULT),
                findings,
            ),
            verbosity: VerbositySection::new(&Verbosity::measure(model), findings),
            import_cycles: ImportCyclesSection::new(&ImportCycles::measure(model), findings),
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
